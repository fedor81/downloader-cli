test-download:
	cargo test download_with_content_length -- --nocapture

test-download-no-length:
	cargo test download_without_content_length -- --nocapture