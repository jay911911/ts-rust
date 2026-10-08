//! Port-only test of the ids that Go's binder gives classes with private
//! names (trunc2, from the trunc1 skeptic repros privdts_o4 and
//! privtail_o3).
//!
//! Go's binder gives each class that names a `#private` member its symbol id
//! (binder/binder.go:326 and :375 `GetSymbolNameForPrivateIdentifier`), in
//! every file and before any checker gives an id. So the ids of a later
//! unique symbol are higher by the number of those classes, and the node
//! builder counts the digits of a late-bound name (`<prefix>@k2@<id>`)
//! toward truncation (checker/nodebuilderimpl.go:2614). Here 120 such
//! classes move `k2` from 2 digits to 3, so `w14` goes. That holds for a
//! `.d.ts` that `skipLibCheck` does not check (`privdts`) and for classes
//! after the use in the checked file (`privtail`). The expected texts are
//! the output of `tsgo-oracle-673a5f17d713 -p tsconfig.json --pretty
//! false`, with and without `--checkers 1`, on the same files
//! (`trunc2/testcase/gen.py`).

use ts_goport::execute::tsc::ExitStatus;

use crate::support::child::run_command_in_child;
use crate::support::runner::TscInput;
use crate::support::test_sys::new_test_sys;

const PROJECT: &str = "/home/src/workspaces/project";

const GLOBALS: &str = "interface Array<T> { length: number; [n: number]: T }
interface Boolean {}
interface CallableFunction {}
interface Function {}
interface IArguments {}
interface NewableFunction {}
interface Number {}
interface Object {}
interface RegExp {}
interface String {}
";

/// The pads of the 4 checked types: names of 6 to 9 `a`s.
const PADS: [usize; 4] = [6, 7, 8, 9];

/// The members `w0` to `w{count - 1}`, each with a space after it.
fn members(count: usize) -> String {
    (0..count).map(|i| format!("w{i}: number; ")).collect()
}

/// For each pad, a unique symbol `k<i>` in a type that is not assignable
/// to `number`.
fn uses() -> String {
    PADS.iter()
        .enumerate()
        .map(|(i, &pad)| {
            format!(
                "declare const k{i}: unique symbol;
declare const x{i}: {{ [k{i}]: void; {}: number; {}q: string }};
const n{i}: number = x{i};
",
                "a".repeat(pad),
                members(20)
            )
        })
        .collect()
}

/// 120 classes with a private member: declared, as TS declaration emit
/// writes them, or with a field that a method reads.
fn classes(declared: bool) -> String {
    (0..120)
        .map(|i| {
            if declared {
                format!("export declare class C{i} {{\n    #private;\n    m{i}(): void;\n}}\n")
            } else {
                format!(
                    "class C{i} {{\n    #p{i} = {i};\n    m{i}() {{ return this.#p{i}; }}\n}}\n"
                )
            }
        })
        .collect()
}

/// The diagnostics of `tsc -p tsconfig.json --pretty false` plus `extra`
/// on `files` (with `globals.d.ts`) and `config`.
fn check(files: &[(&str, String)], config: &str, extra: &[&str]) -> String {
    let mut all = vec![
        (format!("{PROJECT}/globals.d.ts"), GLOBALS.into()),
        (format!("{PROJECT}/tsconfig.json"), config.into()),
    ];
    all.extend(
        files
            .iter()
            .map(|(name, text)| (format!("{PROJECT}/{name}"), text.clone().into())),
    );
    let input = TscInput {
        files: all.into_iter().collect(),
        ..Default::default()
    };
    let sys = new_test_sys(&input, false);
    let mut args: Vec<String> = ["-p", "tsconfig.json", "--pretty", "false"]
        .map(String::from)
        .to_vec();
    args.extend(extra.iter().map(|arg| arg.to_string()));
    let result = run_command_in_child(&sys, &args).unwrap_or_else(|err| panic!("tsgo: {err}"));
    assert!(result.unported.is_none(), "unported {:?}", result.unported);
    assert_eq!(
        result.status,
        ExitStatus::DiagnosticsPresentOutputsGenerated
    );
    // The test system adds the list of files after the diagnostics.
    let output = sys.output_text();
    output
        .split("!!! List files start")
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Go's errors, the first at line `first` of a.ts: the pad-8 type hides one
/// more member than with a 2-digit id (`w14` goes).
fn expected(first: usize) -> String {
    let shown = [15, 15, 14, 14];
    PADS.iter()
        .enumerate()
        .map(|(i, &pad)| {
            format!(
                "a.ts({},7): error TS2322: Type '{{ [k{i}]: void; {}: number; {}... {} more ...; q: string; }}' is not assignable to type 'number'.\n",
                first + 3 * i,
                "a".repeat(pad),
                members(shown[i]),
                20 - shown[i],
            )
        })
        .collect()
}

/// `dep.d.ts`, which `skipLibCheck` leaves unchecked, declares the classes.
fn check_dts(extra: &[&str]) -> String {
    let a = format!(
        "import {{ C0 }} from './dep';\nexport declare const c: C0;\n{}",
        uses()
    );
    check(
        &[("dep.d.ts", classes(true)), ("a.ts", a)],
        r#"{"compilerOptions":{"noLib":true,"skipLibCheck":true,"strict":true,"noEmit":true},"files":["globals.d.ts","dep.d.ts","a.ts"]}"#,
        extra,
    )
}

/// The classes come after the uses in the checked file.
fn check_tail(extra: &[&str]) -> String {
    check(
        &[("a.ts", format!("{}{}", uses(), classes(false)))],
        r#"{"compilerOptions":{"noLib":true,"skipLibCheck":true,"strict":true,"noEmit":true,"target":"es2022"},"files":["globals.d.ts","a.ts"]}"#,
        extra,
    )
}

#[test]
fn private_class_ids_of_an_unchecked_dts_count_as_go() {
    assert_eq!(check_dts(&[]), expected(5));
    assert_eq!(check_dts(&["--checkers", "1"]), expected(5));
}

#[test]
fn private_class_ids_after_the_use_count_as_go() {
    assert_eq!(check_tail(&[]), expected(3));
    assert_eq!(check_tail(&["--checkers", "1"]), expected(3));
}
