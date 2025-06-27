test-download:
	cargo test test_download_content_length -- --nocapture

test-download-no-length:
	cargo test test_download_no_content_length -- --nocapture

shw:
	RUSTFLAGS="-Awarnings" cargo test --quiet test_download_content_length -- --nocapture