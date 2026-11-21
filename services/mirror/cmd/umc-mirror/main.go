package main

import (
	"context"
	"crypto/sha1"
	"crypto/tls"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/signal"
	"path"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"
)

type providerConfig struct {
	ID      string `json:"id"`
	BaseURL string `json:"base_url"`
}

type provider struct {
	base *url.URL
}

type mirror struct {
	cacheDir      string
	maxCacheBytes int64
	maxObjectSize int64
	providers     map[string]provider
	client        *http.Client
	cacheMu       sync.Mutex
	locks         [16]sync.Mutex
	downloads     chan struct{}
	inFlight      chan struct{}
}

func main() {
	logger := slog.New(slog.NewJSONHandler(os.Stdout, nil))
	if err := run(logger); err != nil {
		logger.Error("mirror stopped", "error", err)
		os.Exit(1)
	}
}

func run(logger *slog.Logger) error {
	addr := env("BIND_ADDR", ":8080")
	cacheDir := env("CACHE_DIR", "/data/cache")
	providerFile := env("PROVIDERS_FILE", "/etc/umc-mirror/providers.json")
	maxObjectSize, err := strconv.ParseInt(env("MAX_OBJECT_BYTES", "536870912"), 10, 64)
	if err != nil || maxObjectSize <= 0 {
		return errors.New("MAX_OBJECT_BYTES must be a positive integer")
	}
	maxCacheBytes, err := strconv.ParseInt(env("CACHE_MAX_BYTES", "10737418240"), 10, 64)
	if err != nil || maxCacheBytes <= 0 {
		return errors.New("CACHE_MAX_BYTES must be a positive integer")
	}
	maxConcurrentDownloads, err := strconv.Atoi(env("MAX_CONCURRENT_DOWNLOADS", "4"))
	if err != nil || maxConcurrentDownloads <= 0 {
		return errors.New("MAX_CONCURRENT_DOWNLOADS must be a positive integer")
	}
	if err := requireHTTPS(env("PUBLIC_BASE_URL", "")); err != nil {
		return fmt.Errorf("PUBLIC_BASE_URL: %w", err)
	}
	providers, allowedHosts, err := loadProviders(providerFile)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(cacheDir, 0750); err != nil {
		return fmt.Errorf("create cache directory: %w", err)
	}

	client := &http.Client{
		Timeout: 10 * time.Minute,
		Transport: &http.Transport{
			Proxy:                 nil,
			TLSClientConfig:       &tls.Config{MinVersion: tls.VersionTLS12},
			DialContext:           publicHTTPSDialContext,
			ForceAttemptHTTP2:     true,
			MaxIdleConns:          64,
			MaxIdleConnsPerHost:   8,
			IdleConnTimeout:       90 * time.Second,
			TLSHandshakeTimeout:   10 * time.Second,
			ResponseHeaderTimeout: 30 * time.Second,
		},
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			if len(via) >= 5 {
				return errors.New("too many upstream redirects")
			}
			if req.URL.Scheme != "https" {
				return errors.New("upstream redirect must use HTTPS")
			}
			if _, ok := allowedHosts[strings.ToLower(req.URL.Hostname())]; !ok {
				return errors.New("upstream redirect host is not configured")
			}
			return nil
		},
	}
	app := &mirror{
		cacheDir:      cacheDir,
		maxCacheBytes: maxCacheBytes,
		maxObjectSize: maxObjectSize,
		providers:     providers,
		client:        client,
		downloads:     make(chan struct{}, maxConcurrentDownloads),
		inFlight:      make(chan struct{}, 128),
	}

	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"status":"ok"}`+"\n")
	})
	mux.HandleFunc("GET /v1/objects/", app.serveObject)
	mux.HandleFunc("HEAD /v1/objects/", app.serveObject)
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		http.NotFound(w, r)
	})

	server := &http.Server{
		Addr:              addr,
		Handler:           requestLog(logger, mux),
		ReadHeaderTimeout: 5 * time.Second,
		IdleTimeout:       90 * time.Second,
		MaxHeaderBytes:    16 << 10,
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()
	logger.Info("UMC Mirror listening", "addr", addr, "providers", len(providers), "public_url", env("PUBLIC_BASE_URL", ""))
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}

func loadProviders(filename string) (map[string]provider, map[string]struct{}, error) {
	data, err := os.ReadFile(filename)
	if err != nil {
		return nil, nil, fmt.Errorf("read providers file: %w", err)
	}
	var configs []providerConfig
	if err := json.Unmarshal(data, &configs); err != nil {
		return nil, nil, fmt.Errorf("parse providers file: %w", err)
	}
	if len(configs) == 0 {
		return nil, nil, errors.New("providers file must configure at least one HTTPS provider")
	}
	providers := make(map[string]provider, len(configs))
	hosts := make(map[string]struct{}, len(configs))
	for _, item := range configs {
		if item.ID == "" || strings.ContainsAny(item.ID, "/\\. ") {
			return nil, nil, fmt.Errorf("invalid provider id %q", item.ID)
		}
		parsed, err := url.Parse(item.BaseURL)
		if err != nil || parsed.Scheme != "https" || parsed.Hostname() == "" || parsed.User != nil || parsed.RawQuery != "" || parsed.Fragment != "" {
			return nil, nil, fmt.Errorf("provider %q must have a valid HTTPS base_url without credentials, query, or fragment", item.ID)
		}
		if _, exists := providers[item.ID]; exists {
			return nil, nil, fmt.Errorf("duplicate provider id %q", item.ID)
		}
		providers[item.ID] = provider{base: parsed}
		hosts[strings.ToLower(parsed.Hostname())] = struct{}{}
	}
	return providers, hosts, nil
}

func (m *mirror) serveObject(w http.ResponseWriter, r *http.Request) {
	select {
	case m.inFlight <- struct{}{}:
		defer func() { <-m.inFlight }()
	default:
		http.Error(w, "mirror is busy", http.StatusServiceUnavailable)
		return
	}
	parts := strings.Split(strings.TrimPrefix(r.URL.Path, "/v1/objects/"), "/")
	if len(parts) < 3 || r.URL.RawQuery != "" {
		http.Error(w, "expected /v1/objects/{provider}/{sha1}/{upstream-path}", http.StatusBadRequest)
		return
	}
	providerID, expectedSHA1 := parts[0], strings.ToLower(parts[1])
	if !validSHA1(expectedSHA1) || !safePath(parts[2:]) {
		http.Error(w, "invalid object path or SHA-1", http.StatusBadRequest)
		return
	}
	upstream, ok := m.providers[providerID]
	if !ok {
		http.NotFound(w, r)
		return
	}
	lock := m.lockFor(expectedSHA1)
	lock.Lock()
	defer lock.Unlock()

	cachePath := filepath.Join(m.cacheDir, expectedSHA1[:2], expectedSHA1)
	if f, err := verifiedFile(cachePath, expectedSHA1); err == nil {
		defer f.Close()
		serveFile(w, r, f, expectedSHA1)
		return
	}
	_ = os.Remove(cachePath)
	if r.Method == http.MethodHead {
		http.Error(w, "object is not cached", http.StatusNotFound)
		return
	}
	select {
	case m.downloads <- struct{}{}:
		defer func() { <-m.downloads }()
	case <-r.Context().Done():
		http.Error(w, "request canceled", http.StatusRequestTimeout)
		return
	}

	remotePath := strings.Join(parts[2:], "/")
	target := *upstream.base
	target.Path = path.Join(upstream.base.Path, remotePath)
	request, err := http.NewRequestWithContext(r.Context(), http.MethodGet, target.String(), nil)
	if err != nil {
		http.Error(w, "invalid upstream request", http.StatusBadRequest)
		return
	}
	request.Header.Set("User-Agent", "UMC-Mirror/0.1")
	response, err := m.client.Do(request)
	if err != nil {
		http.Error(w, "upstream request failed", http.StatusBadGateway)
		return
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		status := http.StatusBadGateway
		if response.StatusCode == http.StatusNotFound {
			status = http.StatusNotFound
		}
		http.Error(w, "upstream did not return an object", status)
		return
	}
	if response.ContentLength > m.maxObjectSize {
		http.Error(w, "object exceeds configured size limit", http.StatusRequestEntityTooLarge)
		return
	}

	if err := os.MkdirAll(filepath.Dir(cachePath), 0750); err != nil {
		http.Error(w, "cache is unavailable", http.StatusInsufficientStorage)
		return
	}
	tmp, err := os.CreateTemp(filepath.Dir(cachePath), ".umc-part-*")
	if err != nil {
		http.Error(w, "cache is unavailable", http.StatusInsufficientStorage)
		return
	}
	tmpPath := tmp.Name()
	defer os.Remove(tmpPath)
	hasher := sha1.New()
	written, copyErr := io.Copy(io.MultiWriter(tmp, hasher), io.LimitReader(response.Body, m.maxObjectSize+1))
	if copyErr != nil {
		_ = tmp.Close()
		http.Error(w, "upstream transfer failed", http.StatusBadGateway)
		return
	}
	if written > m.maxObjectSize {
		_ = tmp.Close()
		http.Error(w, "object exceeds configured size limit", http.StatusRequestEntityTooLarge)
		return
	}
	actualSHA1 := hex.EncodeToString(hasher.Sum(nil))
	if actualSHA1 != expectedSHA1 {
		_ = tmp.Close()
		http.Error(w, "upstream object failed SHA-1 verification", http.StatusBadGateway)
		return
	}
	if err := tmp.Sync(); err != nil {
		_ = tmp.Close()
		http.Error(w, "cache write failed", http.StatusInsufficientStorage)
		return
	}
	if err := tmp.Close(); err != nil {
		http.Error(w, "cache write failed", http.StatusInsufficientStorage)
		return
	}
	if written > m.maxCacheBytes {
		http.Error(w, "object exceeds configured cache capacity", http.StatusRequestEntityTooLarge)
		return
	}
	m.cacheMu.Lock()
	if err := makeCacheRoom(m.cacheDir, tmpPath, cachePath, written, m.maxCacheBytes); err != nil {
		m.cacheMu.Unlock()
		http.Error(w, "cache capacity is unavailable", http.StatusInsufficientStorage)
		return
	}
	if err := os.Rename(tmpPath, cachePath); err != nil {
		m.cacheMu.Unlock()
		http.Error(w, "cache write failed", http.StatusInsufficientStorage)
		return
	}
	m.cacheMu.Unlock()
	f, err := os.Open(cachePath)
	if err != nil {
		http.Error(w, "cache read failed", http.StatusInsufficientStorage)
		return
	}
	defer f.Close()
	serveFile(w, r, f, expectedSHA1)
}

func serveFile(w http.ResponseWriter, r *http.Request, f *os.File, digest string) {
	info, err := f.Stat()
	if err != nil {
		http.Error(w, "cache read failed", http.StatusInsufficientStorage)
		return
	}
	w.Header().Set("Content-Type", "application/octet-stream")
	w.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
	w.Header().Set("ETag", `"sha1-`+digest+`"`)
	http.ServeContent(w, r, digest, info.ModTime(), f)
}

func verifiedFile(filename, expected string) (*os.File, error) {
	f, err := os.Open(filename)
	if err != nil {
		return nil, err
	}
	hasher := sha1.New()
	if _, err := io.Copy(hasher, f); err != nil {
		_ = f.Close()
		return nil, err
	}
	if hex.EncodeToString(hasher.Sum(nil)) != expected {
		_ = f.Close()
		return nil, errors.New("cached object hash mismatch")
	}
	if _, err := f.Seek(0, io.SeekStart); err != nil {
		_ = f.Close()
		return nil, err
	}
	return f, nil
}

func (m *mirror) lockFor(key string) *sync.Mutex {
	index := int(key[0] % byte(len(m.locks)))
	return &m.locks[index]
}

type cacheEntry struct {
	path    string
	modTime time.Time
	size    int64
}

func makeCacheRoom(root, tempPath, destination string, incoming, maxBytes int64) error {
	var entries []cacheEntry
	var total int64
	err := filepath.WalkDir(root, func(filename string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() || filename == tempPath || filename == destination || strings.HasPrefix(filepath.Base(filename), ".umc-part-") {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return nil
		}
		entries = append(entries, cacheEntry{path: filename, modTime: info.ModTime(), size: info.Size()})
		total += info.Size()
		return nil
	})
	if err != nil {
		return err
	}
	if total+incoming <= maxBytes {
		return nil
	}
	slices.SortFunc(entries, func(a, b cacheEntry) int { return a.modTime.Compare(b.modTime) })
	for _, entry := range entries {
		if err := os.Remove(entry.path); err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
		total -= entry.size
		if total+incoming <= maxBytes {
			return nil
		}
	}
	if total+incoming > maxBytes {
		return errors.New("cache quota cannot fit object")
	}
	return nil
}

func safePath(parts []string) bool {
	for _, part := range parts {
		if part == "" || part == "." || part == ".." || strings.ContainsAny(part, "\\\x00") {
			return false
		}
	}
	return true
}

func validSHA1(value string) bool {
	if len(value) != sha1.Size*2 {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func requireHTTPS(raw string) error {
	parsed, err := url.Parse(raw)
	if err != nil || parsed.Scheme != "https" || parsed.Hostname() == "" || parsed.User != nil || parsed.RawQuery != "" || parsed.Fragment != "" {
		return errors.New("must be an HTTPS URL with a host and no credentials, query, or fragment")
	}
	return nil
}

func publicHTTPSDialContext(ctx context.Context, network, address string) (net.Conn, error) {
	host, port, err := net.SplitHostPort(address)
	if err != nil {
		return nil, err
	}
	addresses, err := net.DefaultResolver.LookupIPAddr(ctx, host)
	if err != nil {
		return nil, err
	}
	dialer := &net.Dialer{Timeout: 10 * time.Second, KeepAlive: 30 * time.Second}
	var lastErr error
	for _, address := range addresses {
		ip := address.IP
		if !ip.IsGlobalUnicast() || ip.IsPrivate() || ip.IsLoopback() || ip.IsLinkLocalUnicast() || ip.IsUnspecified() {
			continue
		}
		conn, dialErr := dialer.DialContext(ctx, network, net.JoinHostPort(ip.String(), port))
		if dialErr == nil {
			return conn, nil
		}
		lastErr = dialErr
	}
	if lastErr != nil {
		return nil, lastErr
	}
	return nil, errors.New("upstream resolved to no public IP addresses")
}

func requestLog(logger *slog.Logger, next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		next.ServeHTTP(w, r)
		logger.Info("request", "method", r.Method, "path", r.URL.Path, "duration_ms", time.Since(start).Milliseconds())
	})
}

func env(key, fallback string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return fallback
}
