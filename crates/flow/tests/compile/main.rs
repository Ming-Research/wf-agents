//! The compile, check by check: each test breaks one rule of a small valid flow and says the faults it is given, as a
//! whole. The tests are kept in modules by subject, so that a new test goes into the module of what it tests.

#[path = "../support/mod.rs"]
mod support;

mod blocks;
mod cells;
mod conditions;
mod files;
mod named_tests;
mod valid;
