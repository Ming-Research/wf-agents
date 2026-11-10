//! What a work does next.
//!
//! This crate is the deciding code of the engine (principle `decide-purely`): given the flow, the configuration a work
//! runs on and the work's records, it says which cell runs and because of what, or what the work waits for and who can
//! give it. It reads no database, clock or network and changes nothing, and it depends on no other part of the engine
//! (principle `dependencies-one-way`).
