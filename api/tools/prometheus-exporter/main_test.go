package main

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

const samplePool = `{
  "apiVersion":"1",
  "source":"snapshot",
  "updatedAt":"2026-11-09T12:00:00Z",
  "ageSeconds":30,
  "stale":false,
  "pool":{"hashrateHps":1200000000000,"workers":3,"bestShare":42.5,"blocksFound":1,"readyAtLastCheck":true},
  "node":{"initialBlockDownload":false,"stale":false}
}`

func TestScrapeExportsFreshPoolMetrics(t *testing.T) {
	api := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path != "/pool" {
			t.Errorf("unexpected path: %s", request.URL.Path)
		}
		writer.Header().Set("Content-Type", "application/json")
		_, _ = writer.Write([]byte(samplePool))
	}))
	defer api.Close()

	collector := newExporter(api.URL)
	if err := collector.scrape(context.Background()); err != nil {
		t.Fatal(err)
	}
	metrics := collector.metrics()
	for _, expected := range []string{
		"bdl_pool_api_up 1",
		"bdl_pool_snapshot_stale 0",
		"bdl_pool_stratum_v1_ready_at_last_check 1",
		"bdl_pool_hashrate_hps 1200000000000",
		"bdl_pool_workers 3",
		"bdl_pool_blocks_found 1",
		"bdl_pool_bitcoin_initial_block_download 0",
	} {
		if !strings.Contains(metrics, expected) {
			t.Errorf("metrics missing %q:\n%s", expected, metrics)
		}
	}
	if strings.Contains(metrics, "address") || strings.Contains(metrics, "worker=") {
		t.Fatalf("metrics contain identifying labels:\n%s", metrics)
	}
}

func TestFailedScrapeMarksCachedSnapshotStale(t *testing.T) {
	api := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		writer.Header().Set("Content-Type", "application/json")
		_, _ = writer.Write([]byte(samplePool))
	}))
	collector := newExporter(api.URL)
	if err := collector.scrape(context.Background()); err != nil {
		t.Fatal(err)
	}
	api.Close()

	if err := collector.scrape(context.Background()); err == nil {
		t.Fatal("expected scrape failure after API server closed")
	}
	metrics := collector.metrics()
	for _, expected := range []string{
		"bdl_pool_api_up 0",
		"bdl_pool_snapshot_stale 1",
		"bdl_pool_stratum_v1_ready_at_last_check 0",
		"bdl_pool_hashrate_hps 1200000000000",
	} {
		if !strings.Contains(metrics, expected) {
			t.Errorf("metrics missing %q after failed scrape:\n%s", expected, metrics)
		}
	}
}

func TestScrapeRejectsTrailingJSON(t *testing.T) {
    for _, suffix := range []string{" garbage", "\n{}"} {
        t.Run(suffix, func(t *testing.T) {
            api := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
                _, _ = w.Write([]byte(samplePool + suffix))
            }))
            defer api.Close()
            collector := newExporter(api.URL)
            if err := collector.scrape(context.Background()); err == nil { t.Fatal("accepted trailing data after pool JSON") }
            if strings.Contains(collector.metrics(), "bdl_pool_api_up 1") { t.Fatal("invalid response marked API up") }
        })
    }
}

func TestScrapeRejectsOversizedResponse(t *testing.T) {
	api := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		_, _ = writer.Write([]byte(`{"apiVersion":"1","padding":"` + strings.Repeat("a", 2*maxResponseBytes) + `"}`))
	}))
	defer api.Close()
	collector := newExporter(api.URL)
	if err := collector.scrape(context.Background()); err == nil {
		t.Fatal("accepted a pool response larger than the limit")
	}
	if !strings.Contains(collector.metrics(), "bdl_pool_api_up 0") {
		t.Fatal("an oversized response must mark the API as down")
	}
}
