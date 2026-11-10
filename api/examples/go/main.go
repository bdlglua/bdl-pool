package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"
)

type healthResponse struct {
	APIVersion string `json:"apiVersion"`
	Status     string `json:"status"`
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run() error {
	baseURL := os.Getenv("BDL_POOL_API_URL")
	if baseURL == "" {
		baseURL = "https://api.bdl-pool.com:36018/api/v1"
	}
	baseURL = strings.TrimRight(baseURL, "/")

	ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, baseURL+"/health", nil)
	if err != nil {
		return err
	}

	response, err := http.DefaultClient.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf("API returned %s", response.Status)
	}

	var health healthResponse
	decoder := json.NewDecoder(response.Body)
	if err := decoder.Decode(&health); err != nil {
		return fmt.Errorf("decode health response: %w", err)
	}
	var extra json.RawMessage
	if err := decoder.Decode(&extra); err != io.EOF {
		return fmt.Errorf("decode health response: trailing data")
	}
	if health.APIVersion != "1" || health.Status != "ok" {
		return fmt.Errorf("API returned an invalid health response")
	}

	fmt.Printf("BDL Pool API %s: %s\n", health.APIVersion, health.Status)
	return nil
}
