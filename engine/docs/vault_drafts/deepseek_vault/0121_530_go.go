func isLocalURL(baseURL string) bool {
host := strings.ToLower(u.Hostname())
return host == "localhost" || host == "127.0.0.1" ||
host == "0.0.0.0" ||
strings.HasPrefix(host, "192.168.") ||
strings.HasPrefix(host, "10.")
}
