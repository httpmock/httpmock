mod body_conversion_tests;
mod builder_compatibility_test;
mod extensions_test;
#[cfg(feature = "remote")]
mod large_body_test;
mod loop_test;
mod raw_request_test;
#[cfg(feature = "record")]
mod recording_save_test;
mod runtimes_test;
#[cfg(feature = "standalone")]
mod standalone_binary_test;
