package main

import (
    "net/http"
    "net/http/httptest"
    "testing"
)

func TestHealthResponseMustBeOneCompleteJSONDocument(t *testing.T) {
    valid := "{\"apiVersion\":\"1\",\"status\":\"ok\"}"
    for _, tc := range []struct{name, body string; valid bool}{
        {"valid", valid, true},
        {"whitespace", valid+"\n  ", true},
        {"garbage", valid+" garbage", false},
        {"second_document", valid+"\n{}", false},
    } {
        t.Run(tc.name, func(t *testing.T) {
            api := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
                _, _ = w.Write([]byte(tc.body))
            }))
            defer api.Close()
            t.Setenv("BDL_POOL_API_URL", api.URL)
            err := run()
            if (err == nil) != tc.valid { t.Fatalf("run() error = %v; want valid=%v", err, tc.valid) }
        })
    }
}
