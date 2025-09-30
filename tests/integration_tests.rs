// Integration test entry point
// Rust requires integration tests to be in the root of tests/ directory

mod integration {
    mod amend_tests;
    mod bitbucket_api_tests;
    mod branch_deletion_safety_tests;
    mod checkout_safety_tests;
    mod config_management_tests;
    mod end_to_end_tests;
    mod force_push_safety_tests;
    mod hook_content_tests;
    mod multi_stack_tests;
    mod network_failure_tests;
    mod platform_tests;
    mod squash_and_push_tests;
    mod test_helpers;
}
