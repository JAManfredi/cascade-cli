// Integration test entry point
// Rust requires integration tests to be in the root of tests/ directory

mod integration {
    mod bitbucket_api_tests;
    mod config_management_tests;
    mod end_to_end_tests;
    mod multi_stack_tests;
    mod network_failure_tests;
    mod squash_and_push_tests;
    mod test_helpers;
}
