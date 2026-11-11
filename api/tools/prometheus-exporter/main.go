package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"
)

const defaultAPIBaseURL = "https://api.bdl-pool.com:36018/api/v1"

// The /pool document is a few hundred bytes; never buffer an unbounded reply.
const maxResponseBytes = 1 << 20

type poolSnapshot struct {
	APIVersion string `json:"apiVersion"`
	Stale      bool   `json:"stale"`
	AgeSeconds *int64 `json:"ageSeconds"`
	UpdatedAt  string `json:"updatedAt"`
	Pool       struct {
		HashrateHps      *float64 `json:"hashrateHps"`
		Workers          *int64   `json:"workers"`
		BestShare        *float64 `json:"bestShare"`
		BlocksFound      *int64   `json:"blocksFound"`
		ReadyAtLastCheck bool     `json:"readyAtLastCheck"`
	} `json:"pool"`
	Node struct {
		InitialBlockDownload *bool `json:"initialBlockDownload"`
		Stale                bool  `json:"stale"`
	} `json:"node"`
}

type exporter struct {
	apiURL string
	client *http.Client
	mu     sync.RWMutex
	state  exporterState
}

type exporterState struct {
	apiUp      bool
	snapshot   *poolSnapshot
	lastScrape time.Time
	lastGood   time.Time
}

func newExporter(apiURL string) *exporter {
	return &exporter{
		apiURL: strings.TrimRight(apiURL, "/"),
		client: &http.Client{Timeout: 8 * time.Second},
	}
}

func validNumber(value *float64) bool {
	return value == nil || (*value >= 0 && !(*value != *value))
}

func (e *exporter) scrape(ctx context.Context) error {
	e.mu.Lock()
	e.state.lastScrape = time.Now()
	e.mu.Unlock()
	fail := func(err error) error {
		e.mu.Lock()
		e.state.apiUp = false
		e.mu.Unlock()
		return err
	}

	request, err := http.NewRequestWithContext(ctx, http.MethodGet, e.apiURL+"/pool", nil)
	if err != nil {
		return fail(err)
	}
	response, err := e.client.Do(request)
	if err != nil {
		return fail(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return fail(fmt.Errorf("pool API returned %s", response.Status))
	}

	var snapshot poolSnapshot
	decoder := json.NewDecoder(io.LimitReader(response.Body, maxResponseBytes))
	if err := decoder.Decode(&snapshot); err != nil {
		return fail(fmt.Errorf("decode pool response: %w", err))
	}
	var extra json.RawMessage
	if err := decoder.Decode(&extra); err != io.EOF {
		return fail(errors.New("pool API returned trailing data after the snapshot"))
	}
	if snapshot.APIVersion != "1" || !validNumber(snapshot.Pool.HashrateHps) || !validNumber(snapshot.Pool.BestShare) {
		return fail(errors.New("pool API returned an invalid snapshot"))
	}
	if snapshot.Pool.Workers != nil && *snapshot.Pool.Workers < 0 {
		return fail(errors.New("pool API returned a negative worker count"))
	}
	if snapshot.Pool.BlocksFound != nil && *snapshot.Pool.BlocksFound < 0 {
		return fail(errors.New("pool API returned a negative block count"))
	}

	e.mu.Lock()
	defer e.mu.Unlock()
	e.state.snapshot = &snapshot
	e.state.apiUp = true
	e.state.lastGood = time.Now()
	return nil
}

func writeGauge(builder *strings.Builder, name string, value float64) {
	fmt.Fprintf(builder, "%s %s\n", name, strconv.FormatFloat(value, 'f', -1, 64))
}

func unixOrZero(value time.Time) int64 {
	if value.IsZero() {
		return 0
	}
	return value.Unix()
}

func (e *exporter) metrics() string {
	e.mu.RLock()
	state := e.state
	e.mu.RUnlock()

	var body strings.Builder
	body.WriteString("# HELP bdl_pool_api_up Whether the most recent BDL Pool API scrape succeeded.\n# TYPE bdl_pool_api_up gauge\n")
	apiUp := 0.0
	if state.apiUp {
		apiUp = 1
	}
	writeGauge(&body, "bdl_pool_api_up", apiUp)
	body.WriteString("# HELP bdl_pool_last_scrape_timestamp_seconds Unix time of the most recent API scrape.\n# TYPE bdl_pool_last_scrape_timestamp_seconds gauge\n")
	writeGauge(&body, "bdl_pool_last_scrape_timestamp_seconds", float64(unixOrZero(state.lastScrape)))
	body.WriteString("# HELP bdl_pool_last_success_timestamp_seconds Unix time of the most recent successful API scrape.\n# TYPE bdl_pool_last_success_timestamp_seconds gauge\n")
	writeGauge(&body, "bdl_pool_last_success_timestamp_seconds", float64(unixOrZero(state.lastGood)))

	snapshot := state.snapshot
	stale := !state.apiUp
	if snapshot != nil {
		stale = stale || snapshot.Stale || snapshot.Node.Stale
	}
	body.WriteString("# HELP bdl_pool_snapshot_stale Whether the last pool or node snapshot is stale or the latest scrape failed.\n# TYPE bdl_pool_snapshot_stale gauge\n")
	if stale {
		writeGauge(&body, "bdl_pool_snapshot_stale", 1)
	} else {
		writeGauge(&body, "bdl_pool_snapshot_stale", 0)
	}

	ready := 0.0
	if state.apiUp && snapshot != nil && !stale && snapshot.Pool.ReadyAtLastCheck {
		ready = 1
	}
	body.WriteString("# HELP bdl_pool_stratum_v1_ready_at_last_check Whether the latest fresh snapshot confirmed a Stratum V1 job.\n# TYPE bdl_pool_stratum_v1_ready_at_last_check gauge\n")
	writeGauge(&body, "bdl_pool_stratum_v1_ready_at_last_check", ready)

	if snapshot == nil {
		return body.String()
	}
	writeOptionalGauge(&body, "bdl_pool_hashrate_hps", "Hashrate in hashes per second.", snapshot.Pool.HashrateHps)
	writeOptionalGauge(&body, "bdl_pool_workers", "Connected workers in the latest pool snapshot.", intPointerToFloat(snapshot.Pool.Workers))
	writeOptionalGauge(&body, "bdl_pool_best_share_difficulty", "Best share difficulty in the latest pool snapshot.", snapshot.Pool.BestShare)
	writeOptionalGauge(&body, "bdl_pool_blocks_found", "Blocks reported by the pool.", intPointerToFloat(snapshot.Pool.BlocksFound))
	if snapshot.Node.InitialBlockDownload != nil {
		value := 0.0
		if *snapshot.Node.InitialBlockDownload {
			value = 1
		}
		body.WriteString("# HELP bdl_pool_bitcoin_initial_block_download Whether Bitcoin Core is still syncing.\n# TYPE bdl_pool_bitcoin_initial_block_download gauge\n")
		writeGauge(&body, "bdl_pool_bitcoin_initial_block_download", value)
	}
	return body.String()
}

func intPointerToFloat(value *int64) *float64 {
	if value == nil {
		return nil
	}
	converted := float64(*value)
	return &converted
}

func writeOptionalGauge(builder *strings.Builder, name, help string, value *float64) {
	if value == nil {
		return
	}
	fmt.Fprintf(builder, "# HELP %s %s\n# TYPE %s gauge\n", name, help, name)
	writeGauge(builder, name, *value)
}

func main() {
	apiURL := os.Getenv("BDL_POOL_API_URL")
	if apiURL == "" {
		apiURL = defaultAPIBaseURL
	}
	listenAddress := os.Getenv("PROMETHEUS_LISTEN_ADDRESS")
	if listenAddress == "" {
		listenAddress = "127.0.0.1:9108"
	}
	intervalSeconds := int64(30)
	if configured := os.Getenv("BDL_POOL_SCRAPE_SECONDS"); configured != "" {
		parsed, err := strconv.ParseInt(configured, 10, 64)
		if err != nil || parsed < 5 || parsed > 86400 {
			log.Fatal("BDL_POOL_SCRAPE_SECONDS must be an integer between 5 and 86400")
		}
		intervalSeconds = parsed
	}

	collector := newExporter(apiURL)
	go func() {
		for {
			if err := collector.scrape(context.Background()); err != nil {
				log.Printf("Pool API scrape failed: %v", err)
			}
			time.Sleep(time.Duration(intervalSeconds) * time.Second)
		}
	}()

	mux := http.NewServeMux()
	mux.HandleFunc("/metrics", func(writer http.ResponseWriter, request *http.Request) {
		if request.Method != http.MethodGet {
			writer.Header().Set("Allow", http.MethodGet)
			http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		writer.Header().Set("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
		writer.Header().Set("X-Content-Type-Options", "nosniff")
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write([]byte(collector.metrics()))
	})
	mux.HandleFunc("/healthz", func(writer http.ResponseWriter, request *http.Request) {
		if request.Method != http.MethodGet {
			http.Error(writer, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		writer.WriteHeader(http.StatusOK)
		_, _ = writer.Write([]byte("ok\n"))
	})

	server := &http.Server{
		Addr:              listenAddress,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
		ReadTimeout:       10 * time.Second,
		WriteTimeout:      10 * time.Second,
		IdleTimeout:       60 * time.Second,
		MaxHeaderBytes:    1 << 14,
	}
	log.Printf("BDL Pool Prometheus exporter listening on %s", listenAddress)
	log.Fatal(server.ListenAndServe())
}
