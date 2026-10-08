#![allow(unused_variables)]
//! Port of Effect-TS/tsgo `internal/typeparser/api_stability_safety.go` at
//! `@effect/tsgo@0.51.1` (`47cb1ed7`): the pre-flight recursion guard of the
//! API stability analysis.
//!
//! PORT: lane effect051 part A ported the types and made every function with
//! its final signature and a stub body that calls the `unported` macro; part B
//! ports the bodies (and removes the `allow`). Signature conventions (as in
//! `api_stability_p1.rs`): analysis and scan methods take
//! `tp: &mut TypeParser<'_>`; Go
//! `apiStabilitySubstitution` is `&ApiStabilitySubstitution`; Go
//! `*apiStabilitySafetyBinding` is `Option<&Rc<ApiStabilitySafetyBinding>>`;
//! Go `*ast.NodeList` is `NodeList`; Go `*ast.FunctionLikeBase` is the
//! function-like `Node`; Go `map[*T]bool` work sets are `&mut FxHashMap<_, bool>`
//! (`scope` is only read: `&FxHashMap<TypeId, bool>`).

use crate::effect::typeparser::*;
use crate::prelude::*;

/// Go `apiStabilitySafety`.
/// apiStabilitySafety is the verdict of the pre-flight recursion guard that runs
/// before a potentially recursive lazy checker read. The guard inspects the
/// represented compiler type graph and the raw declaration structure without
/// performing the guarded resolution itself. Safe authorizes the ordinary lazy
/// read, Recursive and Unknown never do.
///
/// The guard is a pure safety analysis: it decides only whether a read may
/// expand a recursive alias without a lazy memo boundary. It never contributes
/// stability findings and it never suppresses compiler diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ApiStabilitySafety {
    /// Go `apiStabilitySafetySafe`.
    #[default]
    Safe = 0,
    /// Go `apiStabilitySafetyRecursive`.
    Recursive = 1,
    /// Go `apiStabilitySafetyUnknown`.
    Unknown = 2,
}

/// apiStabilitySafetyMaxWork bounds one guard run. Exhausting it yields
/// Unknown, which blocks the read; it is never memoized as safe.
pub const API_STABILITY_SAFETY_MAX_WORK: i32 = 16_384;
/// apiStabilitySafetyMaxDepth bounds nested alias applications on one guard
/// path. Exceeding it yields Unknown.
pub const API_STABILITY_SAFETY_MAX_DEPTH: i32 = 64;

/// Go `apiStabilitySafetyOperation`.
/// apiStabilitySafetyOperation identifies the guarded read family a completed
/// verdict belongs to, so a Safe result is only reused for the same operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ApiStabilitySafetyOperation {
    /// Go `apiStabilitySafetyOperationMemberTable`.
    #[default]
    MemberTable = 0,
    /// Go `apiStabilitySafetyOperationBaseTypes`.
    BaseTypes = 1,
    /// Go `apiStabilitySafetyOperationSymbolType`.
    SymbolType = 2,
    /// Go `apiStabilitySafetyOperationSignatureMembers`.
    SignatureMembers = 3,
    /// Go `apiStabilitySafetyOperationSignatureReturn`.
    SignatureReturn = 4,
    /// Go `apiStabilitySafetyOperationDeclaredType`.
    DeclaredType = 5,
    /// Go `apiStabilitySafetyOperationAnnotation`.
    Annotation = 6,
}

/// Go `apiStabilitySafetyVerdictKey`.
/// apiStabilitySafetyVerdictKey is the concrete identity of one guard request:
/// the read operation, the represented compiler object or raw node it resolves,
/// and the interned carrier of the substitution it is read under. The carrier
/// is the identity of the whole substitution chain, so two requests that would
/// resolve different represented components never share a verdict. Relation
/// operand and projection scans remain internal to a request, so they cannot
/// collide with a completed top-level verdict. No serialized context, string or
/// raw declaration target is part of a key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ApiStabilitySafetyVerdictKey {
    pub operation: ApiStabilitySafetyOperation,
    pub t: TypeId,
    pub signature: SignatureId,
    pub symbol: SymbolId,
    pub node: Node,
    pub carrier: ApiStabilityCarrierId,
}

/// Go `apiStabilitySafetyArg`.
/// apiStabilitySafetyArg is one concrete argument of a represented generic
/// application. Either the checker already represented the argument type, or the
/// raw argument node is kept together with the environment it was written in so
/// a type parameter argument can still be followed through an outer binding.
#[derive(Clone, Debug, Default)]
pub struct ApiStabilitySafetyArg {
    pub t: TypeId,
    pub node: Node,
    pub subst: ApiStabilitySubstitution,
    pub bindings: Option<Rc<ApiStabilitySafetyBinding>>,
}

impl ApiStabilitySafetyArg {
    // Go: typeparser/api_stability_safety.go apiStabilitySafetyArg.empty
    pub fn empty(&self) -> bool {
        unported!("apiStabilitySafetyArg.empty")
    }
}

// Go: typeparser/api_stability_safety.go safetyArgsEqual
pub fn safety_args_equal(left: &[ApiStabilitySafetyArg], right: &[ApiStabilitySafetyArg]) -> bool {
    unported!("safetyArgsEqual")
}

/// Go `apiStabilitySafetyBinding`.
/// apiStabilitySafetyBinding is one generic-application frame: the declared type
/// parameters of a class, interface or alias bound to the arguments of one
/// application. Frames chain so a nested application keeps the outer bindings.
#[derive(Clone, Debug, Default)]
pub struct ApiStabilitySafetyBinding {
    pub parent: Option<Rc<ApiStabilitySafetyBinding>>,
    pub params: Vec<TypeId>,
    pub args: Vec<ApiStabilitySafetyArg>,
}

/// Go `apiStabilitySafetyApplication`.
/// apiStabilitySafetyApplication is one alias application on the current guard
/// path. The concrete argument list is compared by identity so a repeated
/// application of the same alias with the same represented arguments is
/// recognized as an evaluated cycle.
#[derive(Clone, Debug, Default)]
pub struct ApiStabilitySafetyApplication {
    pub symbol: SymbolId,
    pub args: Vec<ApiStabilitySafetyArg>,
}

/// Go `apiStabilitySafetyScan`.
/// apiStabilitySafetyScan is the mutable state of one guard run. The verdict is
/// computed by traversing raw annotations, represented types and declaration
/// structure; a work and depth bound makes the traversal total and fails closed.
/// All nested scans share one analysis-level work counter so a chain of nested
/// argument verifications cannot run away.
///
/// PORT: Go `a *apiStabilityAnalysis` is a borrow of the analysis for the scan.
/// Go nil maps are empty maps.
pub struct ApiStabilitySafetyScan<'a> {
    pub a: &'a mut ApiStabilityAnalysis,

    pub stack: Vec<ApiStabilitySafetyApplication>,

    // relationOperands marks a nested scan that verifies a conditional operand
    // before the compiler relation is asked to compare it. The relation compares
    // the full projected member surface of both operands (properties, call and
    // construct returns, index infos) and instantiates generic members, so the
    // operand scan evaluates return annotations and refuses any component the
    // relation could instantiate (an unbound type parameter, a deferral, an
    // inferred return).
    pub relation_operands: bool,

    pub active_declarations: FxHashMap<SymbolId, bool>,
    pub materializing: FxHashMap<TypeId, bool>,
    pub materializing_nodes: FxHashMap<Node, bool>,
}

impl ApiStabilitySafetyScan<'_> {
    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.exceeded
    pub fn exceeded(&mut self) -> bool {
        unported!("apiStabilitySafetyScan.exceeded")
    }
}

// Go: typeparser/api_stability_safety.go combineSafety
/// combineSafety merges two verdicts of one read: any recursion makes the whole
/// read recursive, and any unknown makes it unknown.
pub fn combine_safety(left: ApiStabilitySafety, right: ApiStabilitySafety) -> ApiStabilitySafety {
    unported!("combineSafety")
}

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.symbolTypeResolutionIsSafe
    pub fn symbol_type_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
    ) -> bool {
        unported!("apiStabilityAnalysis.symbolTypeResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.signatureMemberResolutionIsSafe
    pub fn signature_member_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: SymbolId,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        unported!("apiStabilityAnalysis.signatureMemberResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.annotationResolutionIsSafe
    pub fn annotation_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        unported!("apiStabilityAnalysis.annotationResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.declaredTypeResolutionIsSafe
    pub fn declared_type_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
    ) -> bool {
        unported!("apiStabilityAnalysis.declaredTypeResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.signatureReturnResolutionIsSafe
    pub fn signature_return_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: SignatureId,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        unported!("apiStabilityAnalysis.signatureReturnResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.inferredComponentResolutionIsSafe
    pub fn inferred_component_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: Node,
    ) -> bool {
        unported!("apiStabilityAnalysis.inferredComponentResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.baseTypesResolutionIsSafe
    pub fn base_types_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: TypeId,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        unported!("apiStabilityAnalysis.baseTypesResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.memberTableResolutionIsSafe
    pub fn member_table_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        unported!("apiStabilityAnalysis.memberTableResolutionIsSafe")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.safetyBudgetExhausted
    pub fn safety_budget_exhausted(&self) -> bool {
        unported!("apiStabilityAnalysis.safetyBudgetExhausted")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.safetySafeVerdict
    pub fn safety_safe_verdict(&self, key: &ApiStabilitySafetyVerdictKey) -> bool {
        unported!("apiStabilityAnalysis.safetySafeVerdict")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.recordSafetySafeVerdict
    pub fn record_safety_safe_verdict(&mut self, key: ApiStabilitySafetyVerdictKey) {
        unported!("apiStabilityAnalysis.recordSafetySafeVerdict")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.symbolAtTypeNameNode
    pub fn symbol_at_type_name_node(&mut self, tp: &mut TypeParser<'_>, node: Node) -> SymbolId {
        unported!("apiStabilityAnalysis.symbolAtTypeNameNode")
    }

    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.newSafetyScan
    pub fn new_safety_scan(&mut self) -> ApiStabilitySafetyScan<'_> {
        unported!("apiStabilityAnalysis.newSafetyScan")
    }
}

impl ApiStabilitySafetyScan<'_> {
    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeNode
    pub fn type_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.typeNode")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeArguments
    pub fn type_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        arguments: NodeList,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.typeArguments")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeReference
    pub fn type_reference(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.typeReference")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.referenceToSymbol
    pub fn reference_to_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        arguments: NodeList,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.referenceToSymbol")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.omittedArgumentDefaults
    pub fn omitted_argument_defaults(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        arguments: NodeList,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.omittedArgumentDefaults")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeParameterReference
    pub fn type_parameter_reference(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.typeParameterReference")
    }
}

// Go: typeparser/api_stability_safety.go safetyBindingLookup
// PORT: Go reads `parameter.Symbol()` without a checker; here it takes `c`.
pub fn safety_binding_lookup(
    c: &Checker,
    bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    parameter: TypeId,
) -> (ApiStabilitySafetyArg, bool) {
    unported!("safetyBindingLookup")
}

impl ApiStabilitySafetyScan<'_> {
    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.aliasApplication
    pub fn alias_application(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        arguments: NodeList,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.aliasApplication")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeValue
    pub fn type_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.typeValue")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.mappedParameterValue
    pub fn mapped_parameter_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> (TypeId, bool) {
        unported!("apiStabilitySafetyScan.mappedParameterValue")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.materializeArgument
    pub fn materialize_argument(
        &mut self,
        tp: &mut TypeParser<'_>,
        argument: &ApiStabilitySafetyArg,
    ) -> TypeId {
        unported!("apiStabilitySafetyScan.materializeArgument")
    }
}

// Go: typeparser/api_stability_safety.go apiStabilityIntrinsicTypeOfNode
pub fn api_stability_intrinsic_type_of_node(c: &Checker, node: Node) -> TypeId {
    unported!("apiStabilityIntrinsicTypeOfNode")
}

impl ApiStabilitySafetyScan<'_> {
    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.constraintSafety
    pub fn constraint_safety(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.constraintSafety")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.objectValue
    pub fn object_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.objectValue")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.mappedValue
    pub fn mapped_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.mappedValue")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.conditionalValue
    pub fn conditional_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.conditionalValue")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.conditionalBranchValue
    pub fn conditional_branch_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        true_branch: bool,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.conditionalBranchValue")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.conditionalNode
    pub fn conditional_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.conditionalNode")
    }
}

// Go: typeparser/api_stability_safety.go branchSelection
// branchSelection is a bit set of conditional branches the compiler relation
// evaluates.
crate::flags_macros::go_flags!(BranchSelection, u8 {
    NONE = 0;
    /// Go `branchUnknown`.
    UNKNOWN = 1 << 0;
    /// Go `branchTrue`.
    TRUE = 1 << 1;
    /// Go `branchFalse`.
    FALSE = 1 << 2;
});

impl ApiStabilitySafetyScan<'_> {
    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.selectConditionalBranch
    pub fn select_conditional_branch(
        &mut self,
        tp: &mut TypeParser<'_>,
        check: TypeId,
        extends: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> BranchSelection {
        unported!("apiStabilitySafetyScan.selectConditionalBranch")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.operandType
    pub fn operand_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> TypeId {
        unported!("apiStabilitySafetyScan.operandType")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.containsUnboundParameter
    pub fn contains_unbound_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        active: &mut FxHashMap<TypeId, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.containsUnboundParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.containsReplacedParameter
    pub fn contains_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        active: &mut FxHashMap<TypeId, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.containsReplacedParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeNodeMentionsUnboundParameter
    pub fn type_node_mentions_unbound_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashMap<Node, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.typeNodeMentionsUnboundParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeNodeMentionsUnboundParameterScoped
    pub fn type_node_mentions_unbound_parameter_scoped(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashMap<Node, bool>,
        scope: &FxHashMap<TypeId, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.typeNodeMentionsUnboundParameterScoped")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.functionLikeMentionsUnboundParameter
    pub fn function_like_mentions_unbound_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        function_like: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashMap<Node, bool>,
        scope: &FxHashMap<TypeId, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.functionLikeMentionsUnboundParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeNodeMentionsInfer
    pub fn type_node_mentions_infer(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        visiting: &mut FxHashMap<Node, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.typeNodeMentionsInfer")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.rawDeclarationsMentionReplacedParameter
    pub fn raw_declarations_mention_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.rawDeclarationsMentionReplacedParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.declarationMentionsReplacedParameter
    pub fn declaration_mentions_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashMap<Node, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.declarationMentionsReplacedParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.functionLikeMentionsReplacedParameter
    pub fn function_like_mentions_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        function_like: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashMap<Node, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.functionLikeMentionsReplacedParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeNodeMentionsReplacedParameter
    pub fn type_node_mentions_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashMap<Node, bool>,
    ) -> bool {
        unported!("apiStabilitySafetyScan.typeNodeMentionsReplacedParameter")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeLiteral
    pub fn type_literal(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.typeLiteral")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.mappedTypeNode
    pub fn mapped_type_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.mappedTypeNode")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.memberAnnotation
    pub fn member_annotation(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.memberAnnotation")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.functionLikeResolution
    pub fn function_like_resolution(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.functionLikeResolution")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.memberSymbolOfDeclaration
    pub fn member_symbol_of_declaration(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: Node,
    ) -> SymbolId {
        unported!("apiStabilitySafetyScan.memberSymbolOfDeclaration")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.memberHasLateBoundName
    pub fn member_has_late_bound_name(&mut self, member: Node) -> bool {
        unported!("apiStabilitySafetyScan.memberHasLateBoundName")
    }
}

// Go: typeparser/api_stability_safety.go apiStabilityWellKnownSymbolName
/// apiStabilityWellKnownSymbolName reports whether an entity-name expression is
/// a `Symbol.<well-known>` reference that resolves to a library unique symbol
/// without checking program code.
pub fn api_stability_well_known_symbol_name(expression: Node) -> bool {
    unported!("apiStabilityWellKnownSymbolName")
}

// Go: typeparser/api_stability_safety.go apiStabilityWellKnownSymbolDisplayName
/// apiStabilityWellKnownSymbolDisplayName renders a well-known `Symbol.<name>`
/// computed-name expression for diagnostics, or "" when the expression is not a
/// well-known symbol reference. It reads the expression's identifiers only and
/// never evaluates it.
pub fn api_stability_well_known_symbol_display_name(expression: Node) -> String {
    unported!("apiStabilityWellKnownSymbolDisplayName")
}

impl ApiStabilityAnalysis {
    // Go: typeparser/api_stability_safety.go apiStabilityAnalysis.symbolHasLateBoundMembers
    pub fn symbol_has_late_bound_members(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
    ) -> bool {
        unported!("apiStabilityAnalysis.symbolHasLateBoundMembers")
    }
}

impl ApiStabilitySafetyScan<'_> {
    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.declarationMembers
    pub fn declaration_members(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        arguments: NodeList,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.declarationMembers")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.declarationMembersOfRepresentedArguments
    pub fn declaration_members_of_represented_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        target: TypeId,
        arguments: &[TypeId],
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.declarationMembersOfRepresentedArguments")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.declarationStructure
    pub fn declaration_structure(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.declarationStructure")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.heritage
    pub fn heritage(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: SymbolId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.heritage")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.memberTable
    pub fn member_table(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: TypeId,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.memberTable")
    }

    // Go: typeparser/api_stability_safety.go apiStabilitySafetyScan.typeQuery
    pub fn type_query(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Node,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        unported!("apiStabilitySafetyScan.typeQuery")
    }
}

// Go: typeparser/api_stability_safety.go nonDistributedParameter
/// nonDistributedParameter normalizes a distributed conditional's cloned check
/// parameter back to its declared binder.
pub fn non_distributed_parameter(c: &Checker, t: TypeId) -> TypeId {
    unported!("nonDistributedParameter")
}

// Go: typeparser/api_stability_safety.go safetyTypeParameterDefaults
/// safetyTypeParameterDefaults returns the declared default annotations of a
/// symbol's type parameters, aligned with the symbol's local type parameter
/// order.
// PORT: Go reads `symbol.Declarations` without a checker; here it takes `c`.
pub fn safety_type_parameter_defaults(c: &Checker, symbol: SymbolId) -> Vec<Node> {
    unported!("safetyTypeParameterDefaults")
}

// Go: typeparser/api_stability_safety.go functionLikeParameterNodes
/// functionLikeParameterNodes returns the declared parameters of a function-like
/// node, or nil when it declares none.
pub fn function_like_parameter_nodes(function_like: Node) -> Vec<Node> {
    unported!("functionLikeParameterNodes")
}

// Go: typeparser/api_stability_safety.go declarationAnnotationNodeOf
/// declarationAnnotationNodeOf returns the type annotation node a declaration
/// resolves, or nil for an inferred declaration.
pub fn declaration_annotation_node_of(declaration: Node) -> Node {
    unported!("declarationAnnotationNodeOf")
}
