match url.host_str() {
Some("github.com") => Box::new(GitHubProvider::new(token)),
Some("gitlab.com") => Box::new(GitLabProvider::new(token)),
_ => Err("Unsupported provider"),
}
