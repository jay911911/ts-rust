#![allow(unused_variables)]
//! Port of Effect-TS/tsgo `internal/typeparser/api_stability.go` lines
//! 2683-5023 at `@effect/tsgo@0.51.1` (`47cb1ed7`): declaration surfaces,
//! heritage, class static and anonymous surfaces, mapped and component
//! surfaces, signatures and substitution, provenance, child boundaries,
//! findings and the inspect and visibility filters. See `api_stability_p1.rs`
//! for the split and the signature conventions.
//!
//! PORT: lane effect051 part A made this file with the final signatures and
//! stub bodies that call the `unported` macro; part C ports the bodies (and
//! removes the `allow`).
//! Go `*ast.NodeList` is `NodeList`; Go `map[*T]bool` work sets are
//! `&mut FxHashMap<_, bool>`; Go free functions that read symbols, types or
//! signatures take `c`.

use crate::effect::typeparser::*;
use crate::prelude::*;

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectDeclarationSurface
    /// collectDeclarationSurface inspects the public surface declared by a class or
    /// interface declaration: its own members, index signatures and signatures,
    /// its type parameters, and (when expanded) its inherited members. The
    /// declaration type passed here is always uninstantiated; the substitution
    /// context carries the represented type arguments of the reference that reached
    /// it.
    pub fn collect_declaration_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: TypeId,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectDeclarationSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.memberSurface
    pub fn member_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: SymbolId,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        unported!("apiStabilityAnalysis.memberSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectDeclarationSignatures
    pub fn collect_declaration_signatures(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        symbol: SymbolId,
        subst: &ApiStabilitySubstitution,
        include_class_constructors: bool,
    ) {
        unported!("apiStabilityAnalysis.collectDeclarationSignatures")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectIndexInfosOfDeclaration
    pub fn collect_index_infos_of_declaration(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: TypeId,
        symbol: SymbolId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectIndexInfosOfDeclaration")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectIndexInfoSurface
    pub fn collect_index_info_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        info: IndexInfoId,
        subst: &ApiStabilitySubstitution,
        owner: SymbolId,
    ) {
        unported!("apiStabilityAnalysis.collectIndexInfoSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectIndexSignatureSurface
    #[allow(clippy::too_many_arguments)]
    pub fn collect_index_signature_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Node,
        key_type: TypeId,
        value_type: TypeId,
        subst: &ApiStabilitySubstitution,
        owner: SymbolId,
    ) {
        unported!("apiStabilityAnalysis.collectIndexSignatureSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectHeritage
    pub fn collect_heritage(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: TypeId,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectHeritage")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectInheritedBase
    pub fn collect_inherited_base(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        base: TypeId,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectInheritedBase")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectClassStaticSurface
    pub fn collect_class_static_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: TypeId,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectClassStaticSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectAnonymousSurface
    pub fn collect_anonymous_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectAnonymousSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectResolvedMemberTableIfMaterialized
    pub fn collect_resolved_member_table_if_materialized(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: TypeId,
        owner: SymbolId,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        unported!("apiStabilityAnalysis.collectResolvedMemberTableIfMaterialized")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectAnonymousIndexInfos
    pub fn collect_anonymous_index_infos(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: TypeId,
        source: TypeId,
        subst: &ApiStabilitySubstitution,
        mapper_subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectAnonymousIndexInfos")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectIndexInfosOfSymbol
    pub fn collect_index_infos_of_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        symbol: SymbolId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectIndexInfosOfSymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectFunctionSignatures
    pub fn collect_function_signatures(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: TypeId,
        source: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectFunctionSignatures")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectMappedSurface
    pub fn collect_mapped_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectMappedSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.componentSurface
    pub fn component_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        unported!("apiStabilityAnalysis.componentSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectObjectComponentSurface
    pub fn collect_object_component_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        unported!("apiStabilityAnalysis.collectObjectComponentSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.signatureSurface
    pub fn signature_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: SignatureId,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        unported!("apiStabilityAnalysis.signatureSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectSignatureSurface
    pub fn collect_signature_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        raw: SignatureId,
        concrete: SignatureId,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        unported!("apiStabilityAnalysis.collectSignatureSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectReturnTypeSurface
    pub fn collect_return_type_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        signature: SignatureId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectReturnTypeSurface")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.returnRepresentedType
    pub fn return_represented_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: SignatureId,
        subst: &ApiStabilitySubstitution,
    ) -> (TypeId, bool) {
        unported!("apiStabilityAnalysis.returnRepresentedType")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.parameterTypeOf
    pub fn parameter_type_of(&mut self, tp: &mut TypeParser<'_>, parameter: SymbolId) -> TypeId {
        unported!("apiStabilityAnalysis.parameterTypeOf")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.parameterRepresentedType
    pub fn parameter_represented_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        parameter: SymbolId,
    ) -> (TypeId, bool) {
        unported!("apiStabilityAnalysis.parameterRepresentedType")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.typeParameterFromAnnotation
    pub fn type_parameter_from_annotation(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
    ) -> TypeId {
        unported!("apiStabilityAnalysis.typeParameterFromAnnotation")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.signatureArguments
    pub fn signature_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: SignatureId,
    ) -> Vec<TypeId> {
        unported!("apiStabilityAnalysis.signatureArguments")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.substitute
    pub fn substitute(
        &mut self,
        tp: &mut TypeParser<'_>,
        subst: &ApiStabilitySubstitution,
        t: TypeId,
    ) -> (TypeId, ApiStabilitySubstitution, bool) {
        unported!("apiStabilityAnalysis.substitute")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.signatureTypeParameter
    pub fn signature_type_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: SignatureId,
        t: TypeId,
    ) -> (TypeId, bool) {
        unported!("apiStabilityAnalysis.signatureTypeParameter")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.signatureBoundTypeParameter
    pub fn signature_bound_type_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: SignatureId,
        t: TypeId,
    ) -> (TypeId, bool) {
        unported!("apiStabilityAnalysis.signatureBoundTypeParameter")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.computeSignatureParameterMap
    pub fn compute_signature_parameter_map(
        &mut self,
        tp: &mut TypeParser<'_>,
        raw: SignatureId,
        concrete: SignatureId,
    ) -> FxHashMap<TypeId, TypeId> {
        unported!("apiStabilityAnalysis.computeSignatureParameterMap")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.parameterReturnType
    pub fn parameter_return_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: SignatureId,
        subst: &ApiStabilitySubstitution,
    ) -> TypeId {
        unported!("apiStabilityAnalysis.parameterReturnType")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectDeclarationProvenance
    pub fn collect_declaration_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Node,
        represented: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectDeclarationProvenance")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectSignatureProvenance
    pub fn collect_signature_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        raw: SignatureId,
        signature_subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectSignatureProvenance")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectTypeNodeProvenance
    pub fn collect_type_node_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        node: Node,
        represented: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectTypeNodeProvenance")
    }
}

// Go: typeparser/api_stability.go apiStabilityRepresentedMembers
pub fn api_stability_represented_members(c: &Checker, t: TypeId) -> Vec<TypeId> {
    unported!("apiStabilityRepresentedMembers")
}

// Go: typeparser/api_stability.go apiStabilityRepresentedContainsType
pub fn api_stability_represented_contains_type(members: &[TypeId], t: TypeId) -> bool {
    unported!("apiStabilityRepresentedContainsType")
}

// Go: typeparser/api_stability.go apiStabilityRepresentedContainsAggregate
pub fn api_stability_represented_contains_aggregate(
    c: &Checker,
    members: &[TypeId],
    t: TypeId,
) -> bool {
    unported!("apiStabilityRepresentedContainsAggregate")
}

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectAggregateTypeNodeProvenance
    pub fn collect_aggregate_type_node_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        list: NodeList,
        represented: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectAggregateTypeNodeProvenance")
    }
}

// Go: typeparser/api_stability.go apiStabilityRepresentedIndexedAccess
pub fn api_stability_represented_indexed_access(c: &Checker, t: TypeId) -> (TypeId, TypeId) {
    unported!("apiStabilityRepresentedIndexedAccess")
}

// Go: typeparser/api_stability.go apiStabilityRepresentedConditionalParts
pub fn api_stability_represented_conditional_parts(c: &Checker, t: TypeId) -> (TypeId, TypeId) {
    unported!("apiStabilityRepresentedConditionalParts")
}

// Go: typeparser/api_stability.go apiStabilityRepresentedSurfaceAcceptsArguments
pub fn api_stability_represented_surface_accepts_arguments(
    c: &Checker,
    represented: TypeId,
) -> bool {
    unported!("apiStabilityRepresentedSurfaceAcceptsArguments")
}

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectTypeArgumentProvenance
    pub fn collect_type_argument_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        arguments: NodeList,
        represented: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectTypeArgumentProvenance")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectFunctionTypeProvenance
    pub fn collect_function_type_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        node: Node,
        represented: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectFunctionTypeProvenance")
    }
}

// Go: typeparser/api_stability.go apiStabilityMemberSymbolForDeclaration
pub fn api_stability_member_symbol_for_declaration(
    c: &Checker,
    members: SymbolTable,
    member: Node,
) -> SymbolId {
    unported!("apiStabilityMemberSymbolForDeclaration")
}

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectTypeLiteralProvenance
    pub fn collect_type_literal_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        node: Node,
        represented: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectTypeLiteralProvenance")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectFunctionSignatureProvenance
    pub fn collect_function_signature_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Node,
        raw: SignatureId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectFunctionSignatureProvenance")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.representedTypeArguments
    pub fn represented_type_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
    ) -> Vec<TypeId> {
        unported!("apiStabilityAnalysis.representedTypeArguments")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.representedElementType
    pub fn represented_element_type(&mut self, tp: &mut TypeParser<'_>, t: TypeId) -> TypeId {
        unported!("apiStabilityAnalysis.representedElementType")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.representedTupleArguments
    pub fn represented_tuple_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
    ) -> Vec<TypeId> {
        unported!("apiStabilityAnalysis.representedTupleArguments")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.recordSymbol
    pub fn record_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        symbol: SymbolId,
    ) {
        unported!("apiStabilityAnalysis.recordSymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.recordSignatureStability
    pub fn record_signature_stability(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        signature: SignatureId,
    ) {
        unported!("apiStabilityAnalysis.recordSignatureStability")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.typeChildBoundarySymbol
    pub fn type_child_boundary_symbol(&mut self, tp: &mut TypeParser<'_>, t: TypeId) -> SymbolId {
        unported!("apiStabilityAnalysis.typeChildBoundarySymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.heritageChildBoundarySymbol
    pub fn heritage_child_boundary_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        base: TypeId,
        target: TypeId,
    ) -> SymbolId {
        unported!("apiStabilityAnalysis.heritageChildBoundarySymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectChildTypeArguments
    pub fn collect_child_type_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
    ) {
        unported!("apiStabilityAnalysis.collectChildTypeArguments")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.collectTaggedHeritageArguments
    pub fn collect_tagged_heritage_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: TypeId,
        subst: &ApiStabilitySubstitution,
        visited: &mut FxHashMap<TypeId, bool>,
    ) {
        unported!("apiStabilityAnalysis.collectTaggedHeritageArguments")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.declarationDeclaringSymbol
    pub fn declaration_declaring_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: Node,
    ) -> SymbolId {
        unported!("apiStabilityAnalysis.declarationDeclaringSymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.memberDeclaringSymbol
    pub fn member_declaring_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: SymbolId,
    ) -> SymbolId {
        unported!("apiStabilityAnalysis.memberDeclaringSymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.inheritedChildBoundarySymbol
    pub fn inherited_child_boundary_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: SymbolId,
        owner: SymbolId,
    ) -> SymbolId {
        unported!("apiStabilityAnalysis.inheritedChildBoundarySymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.inheritedChildBoundarySymbolForDeclaration
    pub fn inherited_child_boundary_symbol_for_declaration(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: Node,
        owner: SymbolId,
    ) -> SymbolId {
        unported!("apiStabilityAnalysis.inheritedChildBoundarySymbolForDeclaration")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.childSignatureBoundary
    pub fn child_signature_boundary(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        signature: SignatureId,
        owner: SymbolId,
    ) -> bool {
        unported!("apiStabilityAnalysis.childSignatureBoundary")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.declarationChildBoundary
    pub fn declaration_child_boundary(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Node,
        owner: SymbolId,
    ) -> bool {
        unported!("apiStabilityAnalysis.declarationChildBoundary")
    }
}

// Go: typeparser/api_stability.go apiStabilityDeclarationBelongsToSymbol
pub fn api_stability_declaration_belongs_to_symbol(
    c: &Checker,
    declaration: Node,
    symbol: SymbolId,
) -> bool {
    unported!("apiStabilityDeclarationBelongsToSymbol")
}

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability.go apiStabilityAnalysis.recordDeclarationStability
    pub fn record_declaration_stability(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Node,
    ) {
        unported!("apiStabilityAnalysis.recordDeclarationStability")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.addFinding
    pub fn add_finding(
        &mut self,
        surface: &mut ApiStabilitySurface,
        key: ApiStabilityFindingKey,
        declared: ApiStabilityDeclaration,
    ) {
        unported!("apiStabilityAnalysis.addFinding")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.declarationRepresentedType
    pub fn declaration_represented_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        declaration: Node,
    ) -> TypeId {
        unported!("apiStabilityAnalysis.declarationRepresentedType")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.declarationAnnotationNode
    pub fn declaration_annotation_node(&self, declaration: Node) -> Node {
        unported!("apiStabilityAnalysis.declarationAnnotationNode")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.conditionalDeclarationIsDeferred
    pub fn conditional_declaration_is_deferred(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
    ) -> bool {
        unported!("apiStabilityAnalysis.conditionalDeclarationIsDeferred")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.typeNodeMentionsTypeParameter
    pub fn type_node_mentions_type_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        visiting: &mut FxHashMap<Node, bool>,
    ) -> bool {
        unported!("apiStabilityAnalysis.typeNodeMentionsTypeParameter")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.shouldInspectSymbol
    pub fn should_inspect_symbol(&mut self, tp: &mut TypeParser<'_>, symbol: SymbolId) -> bool {
        unported!("apiStabilityAnalysis.shouldInspectSymbol")
    }

    // Go: typeparser/api_stability.go apiStabilityAnalysis.shouldInspectMemberDeclaration
    pub fn should_inspect_member_declaration(&self, declaration: Node) -> bool {
        unported!("apiStabilityAnalysis.shouldInspectMemberDeclaration")
    }
}

// Go: typeparser/api_stability.go typeParameterAnnotationNode
pub fn type_parameter_annotation_node(c: &Checker, t: TypeId, constraint: bool) -> Node {
    unported!("typeParameterAnnotationNode")
}

// Go: typeparser/api_stability.go signatureReturnTypeNode
pub fn signature_return_type_node(c: &Checker, signature: SignatureId) -> Node {
    unported!("signatureReturnTypeNode")
}

// Go: typeparser/api_stability.go apiStabilityIndexSignatureKeyNode
pub fn api_stability_index_signature_key_node(declaration: Node) -> Node {
    unported!("apiStabilityIndexSignatureKeyNode")
}

// Go: typeparser/api_stability.go apiStabilityIndexSignatureValueNode
pub fn api_stability_index_signature_value_node(declaration: Node) -> Node {
    unported!("apiStabilityIndexSignatureValueNode")
}

// Go: typeparser/api_stability.go apiStabilityMappedDeclarationHasName
pub fn api_stability_mapped_declaration_has_name(c: &Checker, t: TypeId) -> bool {
    unported!("apiStabilityMappedDeclarationHasName")
}

// Go: typeparser/api_stability.go apiStabilityTypeAliasDeclaration
pub fn api_stability_type_alias_declaration(c: &Checker, symbol: SymbolId) -> Node {
    unported!("apiStabilityTypeAliasDeclaration")
}

// Go: typeparser/api_stability.go apiStabilitySymbolDeclaresHeritage
pub fn api_stability_symbol_declares_heritage(c: &Checker, symbol: SymbolId) -> bool {
    unported!("apiStabilitySymbolDeclaresHeritage")
}

// Go: typeparser/api_stability.go sortedSymbolTableKeys
// PORT: Go `sort.Strings` compares Go bytes (`scanner_util::compare_go_strings`).
pub fn sorted_symbol_table_keys(c: &Checker, table: SymbolTable) -> Vec<String> {
    unported!("sortedSymbolTableKeys")
}

// Go: typeparser/api_stability.go apiStabilityMemberTableNameIsVisible
pub fn api_stability_member_table_name_is_visible(
    c: &Checker,
    name: &str,
    member: SymbolId,
) -> bool {
    unported!("apiStabilityMemberTableNameIsVisible")
}

// Go: typeparser/api_stability.go apiStabilitySymbolIsNonPublic
pub fn api_stability_symbol_is_non_public(c: &mut Checker, symbol: SymbolId) -> bool {
    unported!("apiStabilitySymbolIsNonPublic")
}

// Go: typeparser/api_stability.go apiStabilitySymbolHasOnlyInternalDeclarations
pub fn api_stability_symbol_has_only_internal_declarations(c: &Checker, symbol: SymbolId) -> bool {
    unported!("apiStabilitySymbolHasOnlyInternalDeclarations")
}

// Go: typeparser/api_stability.go apiStabilityPropertyDeclarationIsInternal
pub fn api_stability_property_declaration_is_internal(declaration: Node) -> bool {
    unported!("apiStabilityPropertyDeclarationIsInternal")
}

// Go: typeparser/api_stability.go apiStabilitySignatureIsNonPublic
pub fn api_stability_signature_is_non_public(c: &Checker, signature: SignatureId) -> bool {
    unported!("apiStabilitySignatureIsNonPublic")
}
