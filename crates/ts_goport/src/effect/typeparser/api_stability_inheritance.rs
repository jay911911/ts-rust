#![allow(unused_variables)]
//! Port of Effect-TS/tsgo `internal/typeparser/api_stability_inheritance.go` at
//! `@effect/tsgo@0.51.1` (`47cb1ed7`).
//!
//! PORT: lane effect051 part A made this file with the final signatures and
//! stub bodies that call the `unported` macro; part B ports the bodies (and
//! removes the `allow`).

use crate::effect::typeparser::*;
use crate::prelude::*;

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability_inheritance.go apiStabilityAnalysis.optionalTaggedBase
    /// optionalTaggedBase reads declaration metadata only. Every public member,
    /// including inherited and merged members, must be an optional property or
    /// method with its own recognized stability tag. Internal members are ignored.
    /// Call, construct and index signatures cannot be optional. Unknown bases fail
    /// closed; member types are
    /// never evaluated merely to decide whether to exempt an inherited base.
    pub fn optional_tagged_base(&mut self, tp: &mut TypeParser<'_>, symbol: SymbolId) -> bool {
        unported!("apiStabilityAnalysis.optionalTaggedBase")
    }

    // Go: typeparser/api_stability_inheritance.go apiStabilityAnalysis.optionalTaggedInheritedMember
    /// optionalTaggedInheritedMember applies the base exception to properties the
    /// compiler flattened into a derived member table, including superclass factory
    /// intersection types. Own members are always inspected independently.
    pub fn optional_tagged_inherited_member(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: SymbolId,
        owner: SymbolId,
    ) -> bool {
        unported!("apiStabilityAnalysis.optionalTaggedInheritedMember")
    }
}
