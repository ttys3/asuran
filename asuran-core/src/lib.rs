/*!
This crate contains data structures that can be readily shared between both
synchronous and non-synchronous implementations of asuran.

When a data structure is present in this crate, and it has a
Serialize/Deserialize derive, the format that `rmp-serde` produces from
serializing that structure with the compact representation is considered to be
the iconically format of that objects on-disk representation.
*/

#![deny(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::pub_enum_variant_names)]
#![allow(clippy::if_not_else)]
#![allow(clippy::similar_names)]
#![allow(clippy::use_self)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
// Temporary, will remove
#![allow(clippy::cast_possible_truncation)]
pub mod manifest;
pub mod repository;
