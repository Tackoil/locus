# Locus Specification 0 beta 6

*A relational language and CLI toolchain for plane-geometry figures and technical diagrams with precise alignment and ratios.*

This document is self-contained and normative. The project is under development and supports this draft.

This revision incorporates the beta 6 proposal into the normative language and toolchain contract. The [relation diagram](reference/relations.svg), its [Locus source](reference/relations.locus), and both [composition examples](../examples/composition) are formal reference material. It adds reusable functions, constructor declarations, ordered groups, automatic box sizing, standalone and path-attached labels, automatic connector routing, preset measurements, and rectangular boundary edges. The draft remains unfrozen pending the numerical and implementation contracts listed in Part 12. The measured runtime and 28-source acceptance now implement the contracts pinned in §7.7, §8.7 and §9. Remaining freeze questions stay listed in Part 12.

---

## 0. Design principles (informative)

1. **The agent declares geometry and relationships; Locus owns placement.** Objects carry degrees of freedom; relations consume them; a deterministic solver finds the coordinates internally. Points may be declared with `point()` or derived from anchors and geometric constructions.
2. **The `.locus` file is the figure's source code** — ordered, diffable, hand-editable, self-testing (asserts live in the file).
3. **Text-based verification is the primary feedback channel.** `describe` / `inspect` / `check` must be complete enough that a vision-less agent can catch every mistake, including wrong solution branches and leftover degrees of freedom.
4. **Determinism is normative.** Identical file → identical solve → identical resolved geometry → byte-identical SVG.
5. **Statement-per-line syntax with compiler-grade diagnostics.** Local errors stay local; error messages name source lines and suggest fixes.

### 0.1 Current model (informative)

The current draft uses a relational model, keeps placement initialization inside Locus, provides parameterless `point()`, explicit or automatic shape dimensions, and measured labels, makes circle tangency choices explicit, and includes ellipses and B-splines:

1. Geometry and content are declared by their intrinsic properties (`point()`, `circle(2)`, `rect(4, 2)`, `rect()`, `label("text")`); their placement belongs to Locus. Objects are related through measured bounds, points, and anchors.
2. Relations between objects (`tangent`, `on`, `align_center_y`, `dist(A,B) == 6`, …) are first-class statements that *define* geometry, not just check it.

This is the geometric-constraint paradigm proven by CAD sketchers (SolveSpace, FreeCAD/planegcs). The constructive constructors survive as the **derived tier** — direct evaluation, zero unknowns — under the doctrine: **construct when you can, relate when you must.**

### 0.2 Diagram-authoring additions (informative)

The [agent-loop example](../examples/composition/flowchart_agent_loop.locus) and [checkout example](../examples/composition/checkout_sequence.locus) use generic drawing objects and composition. Business meaning comes from text and local functions.

| Addition | Required behavior | Defined in |
|---|---|---|
| Functions, constructor declarations, and groups | Expand named local composition and ordered references into the shared figure. | §1.4–§1.6 |
| Automatic shape dimensions | `rect()`, `diamond()`, and `rounded_rect()` obtain compact dimensions from measured content, relations, and preset defaults. | §2.2, §5.5 |
| Independent and connector labels | `label(text)` participates in layout; `label(path, text)` follows a connector with `near` and page-relative `side` preferences. | §5.6–§5.7, §8.1 |
| Typography roles | `role: title` selects preset title typography without assigning a position. | §5.7, §8.2 |
| Automatic routes | `route: curved` and `route: orthogonal` preserve endpoints while the engine chooses controls and detours. | §5.7, §8.6 |
| Preset measurements | `style_metric("short_length")` and `style_metric("narrow_width")` return length values. | §6.2 |
| Boundary and path measurements | `edge(rect, direction)` derives a finite side; an arrow's `.length` measures its untrimmed centerline. | §5.8, §6.2 |
| Integrated layout and feedback | Text measurement, sizing, separation, routing, and clipping produce one validated scene and source-anchored reports. | §8.6, §9.3–§9.4 |

Initialization, numerical layout, routing search, and preset constants are specified in §7.7–§8.7. The geometric equality algorithm is retained in §7.1. The unified vocabulary in Part 3 is authoritative; remaining freeze questions are listed in Part 12.

---

## Part 1 — The Locus language

### 1.1 File identity

- Extension: `*.locus`; encoding UTF-8; newline-terminated statements.
- Comments: `//` to end of line. Outside strings and TeX spans, a comment may occupy a whole line or follow any statement.
- Header: `locus 0 beta 6`. This exact header is mandatory.
- The file stores the **relational program**. Resolved geometry is always derived by solving + evaluation.
- Machine interchange: `locus fmt --json` emits the parsed AST as JSON (§9.4). The DSL is the authoring format; JSON is derived, never authored.

### 1.2 File shape

```locus
locus 0 beta 6                      // format version header, mandatory, first statement

meta title = "Externally tangent circles"

style textbook               // optional; defaults to textbook
class faint { stroke: dash(5, 3), weight: 0.75, accent: #6B7280 }   // optional style classes (§8.3)

// ---- objects ----
circle c1(2)
circle c2(3) where { align_center_y(c1, c2) }   [highlight]
point O_1 = c1.center               // = rhs names a derived value
point O_2 = c2.center
line  ax(O_1, O_2)   [faint]

// ---- relations ----
rel tangent(c1, c2)

// ---- annotation ----
label l1(O_1, $O_1$)  { dir: S }

// ---- intent ----
assert dist(O_1, O_2) == 5
```

### 1.3 Grammar (EBNF)

```
file          := header (statement | eol)*
header        := "locus" "0" "beta" "6" eol
statement     := op | ctor_op | group_stmt | fn_def | fn_call | rel | assert | meta | style | class

op            := free_num_op | object_op
free_num_op   := "num" ident "=" "free" expr [where] [attr] [props] eol -- expr is the required numeric seed
object_op     := kind ident "=" rhs [where] [attr] [props] eol -- alias, derived value, or differently named constructor
ctor_op       := kind ident "(" [args] ")" [where] [attr] [props] eol -- call of the constructor named `kind` (§1.4)
group_stmt    := "group" ident "(" group_members ")" eol -- ordered references; no geometry or placement (§1.6)
group_members := member_ref ("," member_ref)*
member_ref    := ident ("." ident)*                  -- existing object/point, group, or function instance
fn_def        := "fn" ident "(" [params] ")" "{" eol (fn_body_stmt | eol)* "}" eol
params        := ident ("," ident)*
fn_body_stmt  := op | ctor_op | group_stmt | fn_call | rel | assert
fn_call       := ident ident "(" [args] ")" [attr] eol -- function name, instance name, positional arguments
rhs           := expr                              -- result type must match kind; never a call of the constructor named `kind` (§1.4)
kind        := "point" | "num" | "line" | "ray" | "segment" | "circle" | "arc" | "ellipse" | "bspline"
             | "polygon" | "rect" | "rounded_rect" | "diamond"
             | "label" | "arrow" | "tick" | "right_angle" | "text_block"

where       := "where" "{" boolexpr ("," boolexpr)* "}"
attr        := "[" ident "]"                       -- built-in/declared class, or a class parameter inside fn
props       := "{" [prop ("," prop)*] "}"
prop        := ident ":" propvalue
propvalue   := expr | string | ident | headtok | hexcolor
headtok     := "->" | "<->" | "-"                  -- dedicated tokens (arrow heads)
hexcolor    := "#" hex_digit hex_digit hex_digit hex_digit hex_digit hex_digit [hex_digit hex_digit] -- #RRGGBB or #RRGGBBAA

rel         := "rel" (boolexpr [props] | trim_op) eol -- props only for a standalone inset (§3.4)
trim_op     := "trim_endpoints" "(" expr "," expr ")" -- rendering operation, not a Boolean predicate (§3.5)
assert      := "assert" boolexpr ["tol" number] eol
meta        := "meta" ident "=" string eol        -- one metadata key/value pair per line
style       := "style" ident eol
class       := "class" ident props eol             -- declared style class (§8.3)

args        := expr ("," expr)*
expr        := term (("+" | "-") term)*
term        := unary (("*" | "/") unary)*
unary       := ["-"] postfix
postfix     := atom ("." ident ["(" args ")"])*    -- property access on any value
atom        := number [unit] | boolean | string | tex_literal
             | funcall | ref | "(" expr ")"
funcall     := ident "(" [args] ")"
ref         := ident
unit        := "deg" | "rad"
boolean     := "true" | "false"

boolexpr    := bool_or
bool_or     := bool_and ("or" bool_and)*
bool_and    := bool_not ("and" bool_not)*
bool_not    := ["not"] bool_primary
bool_primary := cmp | predicate | "(" boolexpr ")"
cmp         := expr ("==" | "<" | ">" | "<=" | ">=") expr
predicate   := ident "(" args ")"                 -- Boolean condition call, e.g. align_center_y(A, B); requires at least one argument

number      := (digits ["." [digits]] | "." digits) [exponent]
exponent    := ("e" | "E") [("+" | "-")] digits
digits      := digit+
digit       := "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
hex_digit   := digit | "A" | "B" | "C" | "D" | "E" | "F"
                   | "a" | "b" | "c" | "d" | "e" | "f"
ident       := [A-Za-z][A-Za-z0-9_']*
text_value  := string | tex_literal
string      := '"' (plain_char | escape | inline_tex)* '"'
escape      := '\\"' | '\\\\' | '\\$' | '\\n'
inline_tex  := "$" tex_chars "$"                  -- TeX embedded in surrounding plain text
tex_literal := "$" tex_chars "$"                  -- an all-TeX text value
COMMENT     := "//" …end of line
eol         := [COMMENT] NEWLINE
```

`plain_char` excludes raw newlines, `"`, `\`, and `$`. `tex_chars` excludes raw newlines and an unescaped `$`; backslashes inside a TeX span are preserved verbatim. These lexical exclusions make a quoted string and a bare TeX literal unambiguous.

Symbolic enum arguments are validated by their call context: the second argument of `edge` accepts the direction literals `N | E | S | W` (§5.8), and class arguments passed to local functions resolve as class names (§1.5). Neither form is an undeclared numeric variable, and neither is shadowed by a same-named object id.

Note the `postfix` production: property access applies to any expression, so `seg(B, D).midpoint` and `rect(...)` anchors compose without intermediate ids. It also resolves function-instance members before ordinary object properties: `approved.payment.path` selects an expanded object, and `init.shape.n` selects a shape member's anchor (§1.5).

### 1.4 Statement rules

- **One simple statement per line.** No semicolons or continuation syntax. A function definition is a block: its opening declaration, each body statement, and its closing brace occupy separate lines (§1.5). Metadata uses `meta key = "value"`, with one key/value pair per statement; additional entries use additional `meta` statements. Parse errors are recovered at the next line boundary, so one bad simple statement never invalidates the file's remaining diagnostics.
- **Comments are trivia.** The lexer removes `//` through the end of its physical line when it occurs outside a quoted string or TeX span; the terminating newline remains for recovery. This makes both comment-only lines and trailing comments valid without adding comment alternatives to every production.
- **Two declaration forms, one type check.** `kind id(args)` calls the constructor that shares the kind's name: `circle c(2)`, `point P()`, `label l(P, "A")`, `rect box()`. For `segment` that constructor is `seg`. `kind id = rhs` is used for an alias (`point O = c.center`), a derived constructor with another name (`point M = midpoint(A, B)`, `line t = tangent_line(E, c, 0)`, `arc m = angle_arc(A, B, C, r)`), an expression, or a `free` num. Use the declaration form corresponding to the right-hand side; diagnostics suggest the appropriate form. The declared kind must match the result type. This cross-check catches agent mistakes early (`point M = seg(A,B)` → immediate type error) and makes files skimmable. Overloads are resolved from the arguments as for any call, and a constructor declaration has exactly the placed/derived status and unknowns of its constructor (§2.2).
- **`free` is reserved for numeric declarations.** A free `num` requires a scalar seed expression directly after `free`, as in `num r = free 1.5`; malformed or missing scalar seeds are local parse errors.
- **Points and object references are values.** `point O = c.center` names a derived anchor and introduces no unknowns. A reference such as `circle other = c` aliases the same geometry; it does not allocate another placement.
- **`point()` takes no arguments.** `point P()` declares an independently placed point; other arities are local errors. Use `point Q = P` to name the same point, or a derived constructor to construct another.
- **Placement-owning constructors occur only as a constructor declaration.** `circle c(2)` is valid; `label(circle(2).center, "O")` is invalid. Declare the object first. This gives each placement a stable source id and prevents hidden unknowns in nested expressions (§2.2).
- **Identifiers**: `[A-Za-z][A-Za-z0-9_']*`, unique within their scope. Top-level objects, groups, classes, functions, and instance names share the file namespace. Function parameters and local members follow §1.5; qualified expanded ids are unique per file. Convention (not enforced): single/primed letters for points (`A`, `B'`, `M_1`), words for derived objects (`tri`, `inc`).
- **Reference order**: an op's `rhs` may only reference ids defined on earlier lines; its `where` clause may also reference the id being declared. `rel` and `assert` statements may reference any id defined above them; `rel` equations constrain *earlier* objects (the solve is simultaneous, §7).
- **`[attr]`** — a style class: one of the built-ins `main` (default) | `auxiliary` | `highlight` | `construction`, or a `class` declared on an earlier line (§8.3). Inside a function, a class parameter may supply this name; a class on a named invocation supplies the fallback described in §1.5. A class may use preset tokens or exact color, width, dash, opacity, cap, and join values.
- **`{props}`** — op-specific fields (closed set per kind, §5.7). A standalone `rel inset_*(A, B)` additionally accepts `{ padding: none }` (§3.4); other relation properties and properties on conjunctions are errors. Unknown props are errors.
- **Text strings may mix plain text and TeX.** Outside a TeX span, quoted strings support `\"`, `\\`, `\$`, and `\n`; `\n` produces a centered multi-line text stack. An unescaped `$` opens or closes an inline TeX span, so `"filter offset $O_2$"` is stored as three ordered segments: plain text, TeX, plain text. Use `\$` for a literal dollar sign in the plain-text portion. An unmatched delimiter is a local syntax error.
- **TeX spans preserve backslashes.** Inside either an inline span or a bare all-TeX literal, TeX source is not processed as string escapes: `"angle $\alpha$"` and `$\triangle ABC$` both deliver the backslash to the TeX engine. Raw newlines and nested unescaped dollar delimiters are invalid. `\$` inside TeX remains TeX source for a rendered dollar and does not close the span.
- **Text is represented as segments.** The AST stores every text value as an ordered list of `{kind: "plain" | "tex", source: string}` segments. A bare `$...$` value is therefore one TeX segment, a string without inline spans is one plain segment, and a mixed string has multiple segments. Formatting preserves segment boundaries and escapes canonically.

### 1.5 Reusable statement functions

Functions follow the composition model in [the agent-loop example](../examples/composition/flowchart_agent_loop.locus) and [the checkout example](../examples/composition/checkout_sequence.locus). A function is a file-local reusable statement block, expanded before solving. It has no return type or return value, and its definition creates no objects until invoked. A call is a named statement, not an expression-valued `funcall` from §1.3. Function bodies use the same two declaration forms as the top level (§1.4); `rect shape()` and `label caption(text)` inside a body are ordinary constructor declarations with automatic dimensions (§5.5) and standalone measured text (§5.6).

**Definition and invocation.** Define a function before invoking it. Parameters are positional, named without type annotations, and substituted as structured arguments at expansion time. Their uses determine the required kinds and numeric dimensions. A call must supply exactly the declared number of arguments, including zero arguments for a parameterless function. Object and anchor arguments refer to existing values; passing an object does not copy it or allocate another placement. Text arguments preserve their plain/TeX segments. A parameter used in `[appearance]` must receive a built-in or previously declared style-class name.

The checkout example's connector illustrates object, text, and class arguments:

```locus
fn connector(source, target, text, path_style, text_style) {
    point tail() { mark: none }
    point tip() { mark: none }
    rel on(tail, source)
    rel on(tip, target)
    rel align_center_y(tail, tip)
    arrow path(tail, tip) [path_style]
    label caption(path, text) [text_style] { near: middle, side: above }
    rel avoid(caption, source, target)
}

// source_line and target_line are previously declared segments.
connector checkout(source_line, target_line, "1. Checkout(cart)", main, highlight)
```

This creates `checkout.tail`, `checkout.tip`, `checkout.path`, and `checkout.caption`, and rewrites the body's relations to those members. For example, `on(tail, source)` becomes `on(checkout.tail, source_line)`, and `avoid(caption, source, target)` becomes `avoid(checkout.caption, source_line, target_line)`. The external segments retain their identities. Another invocation creates fresh local declarations and placements even when its arguments are identical; explicit aliases still share the referenced geometry (§1.4). Each expanded declaration has the same placed/derived status and DOFs as its ordinary declaration; the function and its instance namespace add no unknowns of their own.

The agent-loop example composes content and a shape with separate centering and containment requirements:

```locus
fn labeled_rect(text) {
    rect shape()
    label caption(text)
    rel center(caption, shape)
    rel contain(shape, caption)
}

labeled_rect init("locus init")
labeled_rect render("locus render -o fig.svg") [highlight]
```

`init.shape` and `init.caption` expose the first instance's objects; `render.shape` and `render.caption` expose the second. Their dimensions and label placement follow §5.5–§5.6. An external relation such as `column(init.shape, render.shape)` participates in the same figure-wide layout as the relations expanded inside each call.

**Scope and composition.** Parameters and local declaration/call names must be distinct within a function. They may be reused in other functions or invocation instances. A body reference resolves to a parameter, then to a previously declared local member, then to a file-level value or class declared before the function definition. Call arguments resolve in the caller's scope. Substitution preserves those bindings: it never captures a caller's name, rewrites text, or mistakes an object property such as `.center` for a local identifier. A declaration's `where` clause may also reference that declaration, as at top level.

Bodies may contain declarations of either form, groups (§1.6), named calls to earlier functions, relations, and assertions. They may not declare nested functions, metadata, presets, or style classes. There are no returns, recursion, control flow, default arguments, or variadic parameters. Function names cannot replace built-in constructor, measurement, or relation names. Unknown functions, duplicate names, wrong arity, incompatible argument uses, and unresolved members are source-anchored errors.

Nested calls extend the namespace. In the checkout example, `legend key()` expands its `swatch_entry requests(...)` call as `key.requests`; the sample arrow is `key.requests.sample`. Likewise, `payment_branch approved(...)` exposes `approved.payment.path` and `approved.update.path`. Expand calls at their source position, recursively in body order. All resulting declarations, relations, and assertions participate in the figure's shared solve, checks, and rendering; there is no per-function solve.

**Styles.** A class on an invocation is the fallback class for expanded declarations that have no explicit class. An explicit body class, including a substituted class parameter, takes precedence. For nested calls, an explicit class on the inner invocation replaces the inherited fallback for that subtree. An invocation accepts no property block or `where` clause: properties and declaration-site relations stay on the individual declarations inside the body.

**Instance bounds.** An instance name is a namespace and a layout collection, not a geometric object with its own center, dimensions, or anchors. Its bounds combine its own expanded visible members, including members of nested calls. External objects referenced or passed as arguments are excluded: a connector instance includes its path and caption, not the full bounds of the source and target guides. Hidden points contribute no visible extent to the collection, but remain addressable and usable in geometric relations. A local group is an alias collection, not an owned drawing object; declaring one does not pull its external references into the instance's automatic bounds. Alignment, gap, cover, containment, `row`, and `column` treat an instance as one item with these combined bounds; member-wise operations such as `avoid` and `trim_endpoints` visit the applicable owned members, including those of nested calls (§3.4–§3.5). Geometric constructors and predicates must receive a qualified object member such as `checkout.path` rather than a namespace. Explicit groups can select instance collections or individual qualified members (§1.6).

Expansion diagnostics retain the function-body source location and the invocation chain. CLI and JSON representations must preserve editable definitions and calls alongside qualified expanded identities (§9.4). This is the required language behavior; parser, expansion, and reporting support still need implementation and validation (Part 12).

### 1.6 Ordered groups

Groups follow the ordered collection model described in [the agent-loop example](../examples/composition/flowchart_agent_loop.locus) and used in [the checkout example](../examples/composition/checkout_sequence.locus). `group name(member, ...)` aliases existing objects. It draws nothing, creates no objects or placement unknowns, and imposes no layout relation. Members retain their identities, styles, and geometric relationships; an object can belong to several groups without being copied or rendered again.

**Members and order.** A group requires at least one member reference. Each reference must resolve to an already declared drawable object or point, group, or function instance. Qualified references such as `customer.shape` and `approved.payment.path` are accepted. Numeric values, text literals, style classes, function definitions, and constructor calls are not group members. Groups have no style class, properties, `where` clause, geometric anchors, or intrinsic dimensions; relations supply their layout behavior.

Expand nested groups from left to right into their members. An instance reference remains one member: a composed collection whose bounds and member-wise traversal follow §1.5. Keep only the first occurrence of each object or instance identity, including when another name aliases the same object. This normalized member order is stable; it is not sorted by position or changed by solving. References must exist before the group declaration, so forward references and cyclic membership are invalid. Membership is fixed by the declarations, while bounds are recomputed from the members' resolved geometry and measured content.

```locus
// The four header shapes were declared by earlier function calls.
group headers(customer.shape, web.shape, order.shape, gateway.shape)
rel row(headers)

// Nested groups flatten in order and remove repeated object identities.
group first_pair(customer.shape, web.shape)
group all_headers(first_pair, order.shape, web.shape, gateway.shape)
// all_headers has the same normalized members as headers.

// Instance references stay whole: each contributes its owned visible objects as one member.
group loops(approved.update, declined.update)
group headings(approved.heading, declined.heading)
```

**Measured bounds and layout.** A group's bounds are the union of its member measurements, using the same geometry, route, and text measurements as layout relations (§3.4–§3.5). Referencing a path does not include unrelated geometry it references, such as an external guide. An explicitly listed point supplies zero-size bounds even when its mark is hidden; automatic instance collections continue to include only owned visible members. A group supplies no independent center or placement to the solver: its measured center and bounds follow its members.

`row` and `column` expand group arguments into their normalized ordered members before applying alignment and adjacent separation. Thus `row(headers)` arranges four shapes and is valid with one group argument; the expanded sequence must still have at least two items. An instance member stays one item, so `column(approved, declined)` stacks two composed branches rather than interleaving their members. Expansion across arguments retains the first occurrence of each identity. By contrast, center/bound alignment, `gap_x`, `gap_y`, cover, and content containment treat each group argument as one item with combined measured bounds. For example, `gap_y(headers, checkout)` separates the whole header collection from the checkout instance; it does not impose gaps between the headers. A group declaration alone never implies a row, column, alignment, separation, or enclosure.

Clearance checks visit individual members rather than treating the bounding box's empty space as an obstacle. `avoid(headings, guides)` checks the heading labels against the guide members; `avoid(loops, gateway_line.path)` applies to the eligible geometry in each loop instance, here each loop's orthogonal route and caption, while its endpoints stay on their guide. `trim_endpoints(paths, boundary)` visits member paths and preserves their logical endpoints (§3.5). A group has no actual interior or outline, so a containment container or clipping boundary must be a supported closed shape rather than a group. Likewise, `on`, `inside`, tangency, and other geometric predicates do not acquire group overloads; select a member object explicitly.

**Functions and diagnostics.** Groups may be declared inside a function, where their local names expand to qualified ids like other members. A group or instance collection can be passed as a function argument when its uses accept collections: the checkout example passes `paths` to `narrow_overlay`, which applies `trim_endpoints` to that existing collection. Passing or regrouping a collection adds no placement and does not copy its members. Unknown or forward member references, duplicate declaration names, incompatible member kinds, empty group declarations, and collections without measurable members for the requested operation produce source-anchored errors. Diagnostics identify the group and affected member; local groups also retain their invocation chain.

Group grammar, membership, and layout semantics are normative. Parser, measurement, expansion, and CLI/JSON integration follow §1.6, §8.7, and §9.4.

---

## Part 2 — The object model: placed and derived

Geometry is either **placed** (owns an unknown placement) or **derived** (computed from other objects). Numeric parameters and automatic box dimensions may also be unknown. A parameterless `point()` owns its placement just as `circle(r)` owns its center. A standalone measured label owns a layout placement; point-anchored and path-attached labels follow their attachment. A point obtained from an anchor or construction is derived. Placement remains internal in all cases.

### 2.1 Derived tier (direct, zero unknowns)

A derived object is a deterministic `f64` function of previously defined objects (`midpoint(A, B)`, `incircle(A, B, C)`, `foot(P, l)`, …). It introduces no unknowns and has no iterative convergence of its own. Derived definitions are evaluated at every solver trial that needs them and once more for the final scene; a violated construction precondition is a construction error (§7.5). “Direct” means evaluation without an additional solve, not symbolic or infinite-precision arithmetic. **Construct when you can, relate when you must.**

### 2.2 Placed geometry and unknown parameters

The following declarations introduce unknowns:

```locus
point P()                       // 2 internal placement DOFs; no constructor arguments
circle c(2)                     // 2 internal placement DOFs; radius is exactly 2
ellipse e(3, 2)                 // 2 internal placement DOFs; semiaxes 3 and 2
rect box(4, 2)                  // 2 internal placement DOFs; width 4, height 2
diamond decision(3, 2)          // 2 internal placement DOFs; width 3, height 2
num r = free 1.5                // 1 DOF: a numeric parameter, with a scalar seed
```

`circle(r)`, `ellipse(a, b [, theta])`, `rect(w, h)`, `rounded_rect(w, h, r)`, and `diamond(w, h)` each own a center that Locus solves internally. Rectangles, rounded rectangles, and diamonds are axis-aligned; a circle has no orientation parameter. An ellipse's rotation is an angle expression, defaulting to `0deg`, and introduces no implicit orientation unknown. Explicit dimensions are expressions of earlier values, not implicitly free parameters. `circle(r)` with a free numeric `r` therefore depends on three unknown components in total: its two placement components and the separately declared radius parameter.

The zero-argument box overloads add automatic dimensions, and standalone labels add layout placement:

| Declaration | Owned continuous components | Engine-selected values |
|---|---|---|
| `rect box()` / `diamond box()` | center x/y, width, height | compact dimensions and placement |
| `rounded_rect box()` | center x/y, width, height | compact dimensions, placement, and preset-derived corner radius |
| `label caption(text)` | label-box center x/y | placement; text dimensions are measured, not unknown |
| `label caption(anchor, text)` | no independent geometric components | anchor-relative annotation offset |
| `label caption(path, text)` | no independent geometric components | path-relative annotation placement |

Zero-argument box constructors and `label(text)` are declaration-only, including inside function expansion. An automatic rounded rectangle's radius is derived from the preset and resolved dimensions within its radius domain; it is not an extra independent unknown. Router controls and attached-label offsets are internal layout choices. Their integration and reporting follow §8.6 and Part 12.

Anchors such as `c.center`, `box.n`, and `decision.e` expose geometric relationships. Naming an anchor does not allocate unknowns, and constraining it moves its owning geometry (or the upstream objects and parameters on which it depends). `point()` introduces two internal unknowns owned by its declaration; relations constrain them directly. Lines, rays, segments, polygons, arcs, and B-splines derive from existing geometry. Arrows retain existing endpoints and resolve their route; annotations follow their constructor's placement mode (§5.6). A B-spline's controls may be declared points, anchors, or constructed points; the spline derives its geometry from those controls (§5.11).

Explicit-center forms such as `circle(O, r)`, `ellipse(O, a, b [, theta])`, and `rect(C, w, h)` remain derived constructors: their center argument must already be a point value, whether declared with `point()` or derived. They allocate no placement of their own. This permits concentric shapes and constructions at geometric centers.

For the explicit-dimension geometric subset, the equality subsystem's unknown vector concatenates the two internal components of each declared `point()`, the two center components of each placed shape, and the single value of each free `num`, in expanded declaration order (§7.1). Automatic box dimensions and standalone-label placements additionally belong to the joint layout model (§8.6); their numerical integration is to be finalized in Part 12. Derived objects, groups, namespaces, and aliases contribute no independent components. Two `point()` declarations own distinct placements even if relations later make them coincident; `point Q = P` shares P's placement and adds no unknowns.

`num` also has a derived form — a named constant or computed scalar:

```
num fw  = 4
num phi = (1 + sqrt(5)) / 2
```

### 2.3 Placement and solution choices

- **Locus initializes placement.** The engine supplies a deterministic initial placement and settles unconstrained placement internally (§7). A standalone `point P()` or `circle c(2)` is a complete valid figure after the required header.
- **Relations express intent.** Use anchors, alignment, distances, and tangency to relate shapes. Relative measurements such as `hdist(a.center, b.center) == 5` express separation between objects.
- **Discrete choices are geometric.** `tangent(c1, c2)` means external tangency; `internally_tangent(outer, inner)` explicitly requests internal tangency. Multi-valued derived constructions use the documented branch selector (§5).
- **Numeric seeds remain numeric.** `num r = free 1.5` supplies a solver seed for a dimension, angle, ratio, or other numeric parameter. Relations determine the solved value; unresolved numeric freedom settles toward this seed. To fix a radius, write `circle(2)` or pass a constant `num`.
- **Remaining freedoms are reported in terms of objects.** The engine distinguishes harmless rigid movement of a whole figure from unresolved relationships or dimensions (§7.2). Feedback names the affected objects and suggests geometric relations.

Assertions verify the chosen solution; they do not steer placement or select another solution. If a particular ordering or configuration matters, encode it with defining relations or an explicit construction rather than relying on engine placement.

### 2.4 `where` sugar

Relations may be attached at the declaration site; this desugars to standalone `rel` statements immediately following the op:

```locus
circle c2(3) where { align_center_y(c1.center, c2.center), tangent(c1, c2) }
// ≡
circle c2(3)
rel align_center_y(c1.center, c2.center)
rel tangent(c1, c2)
```

A `where` clause may only reference earlier ids (plus the id being declared).

---

## Part 3 — Relations

### 3.1 One relation vocabulary, defining facts and checks

The relation vocabulary follows [the relation diagram](reference/relations.svg). It has **32 entries**, counting equivalent numeric comparison forms and geometric overloads together: the 20 layout and clipping names used by the composition examples (`center`, `contain`, `gap_x`, `gap_y`, `row`, `column`, the four `align_*` bounds, `align_center_x`, `align_center_y`, the four `inset_*`, `cover_x`, `cover_y`, `avoid`, `trim_endpoints`) plus the 12 numeric and geometric entries (`==`, the ordered comparisons, `inside`, `on`, `tangent`, `internally_tangent`, `parallel`, `perpendicular`, `collinear`, `concyclic`, `midpoint`, `between`). The layout additions and their measurement, routing, and solve contracts are normative (§7.7–§8.7).

For a Boolean relation supported in both modes, only the keyword differs:

```
rel    tangent(c1, c2)        -- constrains: the solver must make this true
assert tangent(c1, c2)        -- checks: verified after solving, never solved for
```

**`rel` is an input to the solver; `assert` is an independent test.** A well-written figure uses both: `rel` for the defining facts, `assert` for derived truths that should follow — the figure carries its own tests.

Equality-based geometric relations work in `rel`, `where`, and `assert`. Numeric inequalities, `inside`, and `between` remain assert-only under the current solve contract (§3.3). Measured layout relations define placement, sizing requirements, or clearance; their Boolean checks test the resulting layout. `trim_endpoints` is a rendering operation in `rel`, not a Boolean assertion or a point-placement equation. It preserves the underlying geometry (§3.5).

### 3.2 Unified relation vocabulary

| Relation | Meaning |
|---|---|
| `expr == expr` | Equal numeric measurements with matching dimensions. `length(s) == length(t)` expresses segment congruence. |
| `a < b` / `a > b` / `a <= b` / `a >= b` | Compare numeric measurements with matching dimensions, strictly or inclusively (§7.4). |
| `center(A, B)` | Align both measured centers; two points coincide. Sizing stays independent. |
| `align_center_x(A, B, ...)` | Share a horizontal center; points share an x position. No ordering or separation. |
| `align_center_y(A, B, ...)` | Share a vertical center; points share a y position. No ordering or separation. |
| `align_left(A, B, ...)` | Align left bounds; other placement stays free. |
| `align_right(A, B, ...)` | Align right bounds; other placement stays free. |
| `align_top(A, B, ...)` | Align top bounds; other placement stays free. |
| `align_bottom(A, B, ...)` | Align bottom bounds; other placement stays free. |
| `gap_x(A, B, ...)` | Separate adjacent bounds from left to right with automatic gaps; no alignment. |
| `gap_y(A, B, ...)` | Separate adjacent bounds from top to bottom with automatic gaps; no alignment. |
| `row(A, B, C, ...)` | Left-to-right order and separation with a shared vertical center. |
| `column(A, B, C, ...)` | Top-to-bottom order and separation with a shared horizontal center. |
| `inset_left(A, B)` | Place A at B's inner left margin, or shared border with `padding: none`; y stays free. |
| `inset_right(A, B)` | Place A at B's inner right margin, or shared border with `padding: none`; y stays free. |
| `inset_top(A, B)` | Place A at B's inner top margin, or shared border with `padding: none`; x stays free. |
| `inset_bottom(A, B)` | Place A at B's inner bottom margin, or shared border with `padding: none`; x stays free. |
| `cover_x(A, B, C, ...)` | A covers the combined horizontal extent of the remaining objects with style padding; height and y stay free. |
| `cover_y(A, B, C, ...)` | A covers the combined vertical extent of the remaining objects with style padding; width and x stay free. |
| `contain(B, A, ...)` | Enclose all content's measured bounds in B's actual interior, with style padding. |
| `inside(P, obj)` | P is strictly inside a supported closed region, including a circle, ellipse, or simple polygon; boundary points are excluded. |
| `on(P, obj)` | P lies on supported geometry; closed endpoints count. Ellipses are supported. |
| `avoid(subject, obstacle, ...)` | Keep the subject — a path, label, shape, or collection — clear of every obstacle with automatic clearance. |
| `trim_endpoints(paths, B)` | Clip interior ends of a path or collection of paths to B's outline while retaining the original points. |
| `tangent(c, obj)` | Circle touches a linear object or another circle externally; closed endpoint contact counts. |
| `internally_tangent(outer, inner)` | Inner circle touches outer internally; the outer radius must be strictly larger. |
| `parallel(l1, l2)` | Linear objects have parallel directions; both directions must be nonzero. |
| `perpendicular(l1, l2)` | Linear objects have perpendicular directions; both directions must be nonzero. |
| `collinear(A, B, C, ...)` | All points share the line through distinct A and B; their order is unrestricted. |
| `concyclic(A, B, C, D, ...)` | All points share the circumcircle of three distinct, non-collinear initial points. |
| `midpoint(M, A, B)` | M is halfway between A and B; A–M and M–B have equal lengths. |
| `between(A, B, C)` | B lies strictly between distinct A and C on their line; endpoints are excluded. |

Point coincidence uses `center(P, Q)`; horizontal point alignment uses `align_center_y(P, Q)`, and vertical point alignment uses `align_center_x(P, Q)`. Segment congruence uses numeric length equality. The point constructor `center(c)` (§5.1) remains distinct from the two-argument relation `center(A, B)`.

### 3.3 Geometric residuals and assert-only conditions

The geometric subset retains the following raw residuals and equation counts:

| Relation | Raw residual | Equations |
|---|---|---|
| `expr == expr` | `lhs − rhs` | 1 |
| `center(P, Q)` | `x(P) − x(Q)` and `y(P) − y(Q)` | 2 |
| `align_center_x(P, Q, ...)` | x difference from the first point for each remaining point | n−1 |
| `align_center_y(P, Q, ...)` | y difference from the first point for each remaining point | n−1 |
| `on(P, obj)` | Signed distance to a line, radial difference to a circle, or closest-point distance to a closed ray/segment/arc | 1 |
| `on(P, e)` | Ellipse: `b × (sqrt((u/a)² + (v/b)²) − 1)`, with local components from §5.10; a length-valued implicit residual, not Euclidean distance | 1 |
| `tangent(c, l)` | Closest distance from the circle center to the line or closed ray/segment minus radius | 1 |
| `tangent(c1, c2)` | Center distance minus `r1 + r2` | 1 |
| `internally_tangent(outer, inner)` | Center distance minus `r_outer − r_inner`, requiring `r_outer > r_inner` | 1 |
| `parallel(l1, l2)` | Normalized 2D cross product of direction vectors | 1 |
| `perpendicular(l1, l2)` | Normalized dot product of direction vectors | 1 |
| `collinear(A, B, C, ...)` | Signed perpendicular distance to the line through A and B for each remaining point | n−2 |
| `concyclic(A, B, C, D, ...)` | Radial difference from the circumcircle of A, B, and C for each remaining point | n−3 |
| `midpoint(M, A, B)` | Component differences from `(A+B)/2` | 2 |

Here a **linear object** is a line, ray, or segment. Its direction is the ordered vector from its defining first point to its second point; a zero direction is degenerate. `parallel` and `perpendicular` accept any pair of linear objects. A ray, segment, and arc are closed sets for `on`, so their endpoints count as on the object. Closest-point distance is the ordinary Euclidean distance to that closed set. For `tangent(c, ray|segment)`, endpoint-only contact counts as tangency of the two closed rendered objects; callers that require tangency to the supporting line can additionally assert perpendicularity to the radius.

Circle-circle tangency is selected by the predicate, never by initialization. `tangent(c1, c2)` is symmetric and always external. `internally_tangent(outer, inner)` is ordered: its first circle contains the second, and equal or reversed radii are invalid. The same meaning and domain apply in `rel` and `assert`.

All geometric raw residuals are normalized by §7.4. `collinear` requires its first two points to be distinct. `concyclic` requires its first three points to be distinct and non-collinear. Direction normalization in `parallel`/`perpendicular` divides by both direction lengths. Violating one of these preconditions produces a source-anchored degenerate-geometry error instead of an arbitrary truth value.

Signed axis offsets are expressed through the numeric functions `hdist`/`vdist` (§6.2) under `==`: `rel vdist(b.n, a.s) == 0.8`.

The comparisons `< > <= >=`, `between(A, B, C)`, and `inside(P, obj)` are **assert-only** under the equality solve contract in §7.1. Boolean `or` and `not` are also assert-only because they do not define a single smooth equation set. A geometric `rel` or `where` clause may contain a solvable atomic equality/predicate or an `and` conjunction of them; using an assert-only form there is a semantic error. The new layout requirements in §3.4 are not lowered to ordinary smooth equalities by this contract; their constraint handling remains a prerequisite for freezing (Part 12).

`between(A, B, C)` is strict: it requires `B` to be collinear with distinct `A` and `C`, and the projection parameter of `B` on directed segment `A→C` to lie inside the tolerance-shrunk interval `(ε, 1−ε)`. Endpoints are not between. `inside(P, c)` is also strict and requires `(radius(c) − dist(center(c), P)) / D > ε`; a boundary point is not inside. `inside(P, poly)` requires a valid simple polygon, first rejects points within tolerance of any closed edge, then uses the nonzero winding rule; boundary points are not inside. These definitions use the assert's `ε` from §7.4.

`inside(P, e)` for an ellipse requires the negative of its `on` residual divided by `D` to exceed `ε`; its boundary is excluded. B-splines are open curves and do not accept `inside`. General `on(P, spline)` is not supported in this version: to constrain a point to a selected curve parameter, use `center(P, spline.at(t))`. A free numeric `t` may be solved within the domain `[0, 1]`; it does not create a free geometric point.

### 3.4 Measured alignment, separation, and enclosure

Layout relations use measured axis-aligned bounds and their centers, including actual rendered text extents (§8.2). A point has zero-size bounds. They accept geometric objects and content measured by the renderer. Existing geometry-tier anchors remain subject to §5.8: measured content can participate directly in layout without turning label-box anchors into geometric constructor inputs.

`center` sets two center equalities. Each `align_*` relation aligns only its named bound or center component, using the first argument as the reference for the remaining arguments. These relations permit overlap and do not impose ordering, separation, containment, or equal dimensions. Matching dimensions use ordinary numeric equality.

`gap_x` and `gap_y` require at least two items. Argument order supplies direction: left to right for x, top to bottom for y. Reverse the arguments to reverse the order. A longer list is a chain of adjacent pairs: `gap_y(A, B, C)` means `gap_y(A, B)` and `gap_y(B, C)`. Separation applies to adjacent full measured bounds, not just centers; a hidden point still supplies its zero-size bound. Gaps are chosen automatically from the active style, need not be equal, and are not numeric relation arguments. The other axis remains free. In internal y-up space, top-to-bottom order means decreasing y.

`row(items...)` combines `gap_x(items...)` with `align_center_y(items...)`; `column(items...)` combines `gap_y(items...)` with `align_center_x(items...)`. Neither creates connectors or imposes equal sizes. These combinations apply after collection expansion: a group contributes its normalized ordered members, each function instance counts as one item with combined bounds, and the first occurrence of each identity is retained (§1.5–§1.6). A single group argument is valid when it expands to at least two items. Other alignment, separation, and enclosure relations use each collection's combined measured bounds.

For `inset_*(content, container)`, the first argument is content and the second is its container. The named content bound aligns with the container's corresponding inner margin using style padding; the other axis remains free. A standalone inset may use `{ padding: none }` to align with the container's outer bound instead. This is a symbolic option, not a numeric distance. Two such insets place a corner; one inset plus center alignment places a side center. Insets do not imply full containment.

```locus
rel inset_left(A, B) { padding: none }
rel inset_top(A, B) { padding: none }
```

`cover_x(container, items...)` covers the union of the content's x extents with style padding, leaving height and y placement free; `cover_y` does the same for y, leaving width and x placement free. Each requires a container and at least one content item; items may be shapes, paths, labels, or collections. An automatic container chooses the required extent compactly, so one overlay may span several stacked paths. Covering one axis does not imply containment in the container's actual shape.

`contain(container, content...)` requires the entire measured bounds of every content item to fit inside the container's actual interior with style padding. It accounts for sloping diamond sides, curved boundaries, and other nonrectangular interiors; fitting inside an outer bounding box alone is insufficient. Containment does not imply centering. `center(content, container)` and `contain(container, content)` are separate requirements. `inside(P, obj)` checks strict point membership without style padding; it does not replace measured-content containment.

### 3.5 Clearance and visible endpoint clipping

`avoid(subject, obstacles...)` keeps the listed geometry clear using automatic style clearance. Obstacles are closed shapes (their interiors and outlines), segments and routed paths (their strokes, so crossings and overlaps are avoided), labels (their measured boxes), or collections of these. Collections are traversed member by member (§1.6); avoidance checks actual member geometry rather than empty space inside their combined bounds. Path operations visit the applicable member paths. How the requirement is satisfied depends on the subject:

- A **routed path** subject keeps its source and target attachments while the engine adjusts its route; the obstacles are not moved on its behalf (§8.6).
- A **label** subject keeps its attachment or layout relations while the engine chooses its offset; when the surrounding placement is otherwise unconstrained, layout may widen gaps or margins to make room, as a connector caption can widen guide spacing.
- A **shape or collection** subject applies the rule to each eligible member: paths and labels as above, and placed shapes through their layout placement.

Avoidance concerns actual geometry and routes; it is not implied by alignment or ordered spacing. Routing choices, clearance metrics, and failure diagnostics must be deterministic and finalized in Part 12.

`trim_endpoints(paths, boundary)` changes only visible path ends whose original endpoints lie inside the closed boundary. The visible start becomes the first exit intersection with the outline; the visible end becomes the last entry intersection. Middle crossings remain intact, and arrowheads follow the visible target ends. Original points, anchors, measurements, and their `on` relations remain unchanged. Paths with no interior endpoints are unchanged. A fully enclosed path or incompatible clipping boundaries is diagnosed instead of silently discarded.

Clipping is applied to the resolved path during rendering. It introduces no placement unknowns or geometric equations, and cannot be used as a Boolean condition in `assert`, `and`, `or`, or `not`. Clearance and clipping must participate in render validation and diagnostics; neither changes the mathematical incidence and tangency definitions in §3.3.

Clipping boundaries are the closed regions of circles, ellipses, simple polygons, rectangles, rounded rectangles, and diamonds. Lines, rays, open paths, arcs, groups, and label boxes are not closed-boundary operands. Render-only intersections with these outlines do not add general `intersect` or tangency overloads for curves; their deterministic clipping procedure must be pinned before freezing (Part 12).

---

## Part 4 — Direction, measurement, and internal coordinates

- Internal Cartesian scene space is **y-up**. The renderer flips to screen space; scene coordinates never denote pixels. The origin and absolute locations are engine details, absent from the authoring language and public `describe` / `inspect` output. Viewport is auto-fitted (§8.4).
- Horizontal, vertical, compass directions, and angles express relative geometry and annotation intent.
- `hdist(P, Q) = x(Q) − x(P)`; `vdist(P, Q) = y(Q) − y(P)` (signed).
- Angles are CCW-positive. The unit suffix is **mandatory** on angle literals: `60deg`, `1.2rad`. A bare number in an angle position is a parse error (this kills the degrees/radians hallucination class).
- Scene units are arbitrary; figures state their own scale conventions in `meta` if useful (e.g. "1 unit = 32 ms").

---

## Part 5 — Object kinds and constructors

### 5.1 `point`

```locus
point P()
point Q() where { align_center_y(P, Q), dist(P, Q) == 4 }
point M = midpoint(P, Q)
```

`point()` has exactly zero arguments and introduces two internal placement DOFs. Each
declaration creates a distinct geometric point, not a point fixed at the origin. Its
position is determined by relations and Locus's deterministic defaults. It supports the
usual point marks and styles (§5.7), and may be used anywhere a point value is accepted,
including as a shape's center or a B-spline control. A `where` clause adds relationships
without adding constructor parameters. In the example, `align_center_y` does not specify
which point is to the right; use `hdist(P, Q) == 4` when that ordering matters.

Like other placement-owning constructors, `point()` is declaration-only: nested calls
such as `line(point(), point())` are invalid. Declare and name the points first. The
following constructors and anchor expressions remain derived and introduce no unknowns:

```
midpoint(A, B)
lerp(A, B, t)                   -- t=0 → A, t=1 → B; t may lie outside [0,1]
intersect(o1, o2 [, k])         -- k: branch index, optional when the intersection is unique
foot(P, l)                      -- perpendicular foot of P on line/ray/segment l
project(P, c)                   -- nearest point on circle c to P
rotate(P, O, theta)
reflect(P, axis)                -- axis: line;  reflect(P, O): point reflection
translate(P, from, to)          -- P + (to − from)
on_circle(c, theta)             -- point on c at CCW angle theta from +x
extend(A, B, d)                 -- on ray A→B, at distance dist(A,B) + d from A
centroid(A, B, C, ...)
circumcenter(A, B, C)
incenter(A, B, C)
orthocenter(A, B, C)
center(c)                       -- center of circle/arc/ellipse
```

Diagram placement uses anchors and measured relations such as `row`, `column`,
`gap_x`, `gap_y`, `center`, and `contain`. Numeric `hdist`/`vdist` equalities express
dimensions or data, such as time differences and flow thicknesses. Geometric
constructions such as `lerp`, `translate`, and `on_circle` retain their documented
meaning. Internal routing and annotation displacements remain engine-owned.

An independently declared point may instead be constrained by relations, for example `point T() where { on(T, c1), on(T, c2) }`. Such relations do not distinguish multiple intersections; use an explicit construction branch when that distinction matters. `intersect` omits `k` only when the intersection is unique; when two branches exist and `k` is omitted, that is an error (never a default). Branch ordering for explicit `k`: sorted by the parameter of `o1` — along its direction for a line/ray/segment, by CCW angle from +x for a circle. `locus describe` reports the selected index, defining objects, and distinguishing relative measurements. A non-integer or out-of-range branch is a type error.

### 5.2 `num`

```
num fw = 4                      -- derived: any scalar expression of earlier ids
num r  = free 1.5               -- free: 1 DOF
```

### 5.3 `line`, `ray`, `segment`

```
line(A, B)          ray(A, B)          seg(A, B)
perpendicular(P, l)                 -- line through P ⊥ l
parallel(P, l)                      -- line through P ∥ l
perp_bisector(A, B)
angle_bisector(A, B, C)             -- interior bisector at vertex B
tangent_line(P, c, k)               -- P strictly outside c; k is 0 or 1
```

`tangent_line` always has two branches under its strict-outside precondition, so `k` is mandatory. Branches are sorted by the CCW angle of their contact point on `c`, in `[0deg, 360deg)`; `k = 0` selects the smaller angle and `k = 1` the larger. A non-integer or out-of-range branch is a type error.

The contact point is derived as `foot(c.center, tangent_line(P, c, k))`; no independently placed point is needed.

Segments and arrows share a finite path model. A path has centerline geometry and an independent `head: -> | <-> | -` property. `segment(A, B)` (expression form `seg(A, B)`) constructs an exact straight segment with `head: -` by default; `arrow(A, B)` constructs a connector with `head: ->` and a direct straight route by default. Existing constructor spellings and defaults are preserved.

Adding arrowheads to a segment does not change its endpoints, length, incidence, intersections, projection, ticks, or other geometric operations. A zero-length segment remains valid; arrowheads are omitted when it has no direction. Segments never accept `bend`, `route`, or routing `side` properties. Routed connectors may be straight, curved, or orthogonal; their geometry is not implicitly coerced to a segment, even when a particular route happens to be straight. Both path kinds support attached labels and endpoint clipping. Clipping and arrowheads affect visible rendering, not logical centerline measurements or geometric anchors. Aliases inherit the path's head property unless explicitly overridden.

```locus
segment vector(A, B) { head: -> }
arrow connector(A, B) { head: -, route: orthogonal }
label caption(vector, "Displacement") { near: middle, side: above }
```

### 5.4 `circle`, `arc`

```
circle(r)                           -- placed shape: radius r, center owned by Locus
circle(O, r)                        -- derived circle at an existing point or anchor
circle_through(O, A)
circle_diameter(A, B)
circumcircle(A, B, C)
incircle(A, B, C)
arc(c, P, Q, ccw)                   -- ccw: true|false; P, Q must lie on c (checked)
angle_arc(A, B, C, r)               -- angle-mark arc at vertex B, radius r
```

In either `circle` form, `r` is a length expression, for example `2`, `dist(A, B) / 2`, or a free `num`. `circle(r)` is declaration-only (§1.4); `circle(O, r)` composes as a derived value. Every circle exposes `.center`, including circles whose center was never named by the author.

### 5.5 `polygon`, `rect`, `rounded_rect`, `diamond`

```
polygon(A, B, C, ...)               -- ≥ 3 vertices, listed in order
rect()                             -- placed rectangle with automatic width and height
diamond()                          -- placed rhombus with automatic width and height
rounded_rect()                     -- automatic dimensions and preset-derived corner radius
rect(w, h)                         -- placed axis-aligned rectangle; width w, height h
diamond(w, h)                      -- placed axis-aligned rhombus; width w, height h
rect(C, w, h)                      -- derived rectangle at existing anchor C
diamond(C, w, h)                   -- derived rhombus at existing anchor C
rounded_rect(w, h, r)              -- placed rounded rectangle; circular corner radius r
rounded_rect(C, w, h, r)           -- derived rounded rectangle at existing anchor C
```

The zero-argument forms leave center placement, width, and height to Locus. Their dimensions are positive length values exposed through `.width` and `.height`, so ordinary relations can equate or fix them. No extra `free` declaration or numeric hint is required. Explicit-dimension overloads keep their existing meaning; they are never silently resized to fit content.

`contain(shape, content...)` uses actual measured content and the shape's interior to require sufficient dimensions. `cover_x` and `cover_y` impose extent requirements on the selected axis. Subject to all defining relations, the engine chooses compact dimensions and preset proportions; a standalone automatic box receives positive preset defaults. For equal-sized boxes, every containment requirement participates: the longest caption can enlarge all boxes linked by dimension equalities. A diamond must fit content inside its sloping sides, and a rounded rectangle must respect its corner arcs. `center(content, shape)` remains a separate requirement. When fixed dimensions cannot satisfy containment, checking reports a layout conflict instead of changing the dimensions or hiding overflow. The exact compact-sizing objective, preset proportions, and tie-breaks must be pinned before freezing (Part 12).

A polygon implies its edges for incidence detection, label collision, and rendering. Name a specific edge with `seg(A, B)` and construct regular-polygon vertices with `rotate`.

`rounded_rect` uses the same outer box anchors as `rect`, including bounding-box diagonal anchors (which lie outside a rounded corner when `r > 0`). `.centroid` equals `.center`; `.width`, `.height`, and `.corner_radius` are lengths. Dimensions satisfy `w > 0`, `h > 0`, and `0 <= r <= min(w, h)/2`; invalid radii are rejected rather than clamped. A zero radius gives square corners; the maximum radius gives a capsule, or a circle when the dimensions are equal. The kind remains `rounded_rect` in every case.

Its exact area is `w*h - (4 - π)*r*r`; its perimeter is `2*(w+h) - (8 - 2*π)*r`. `area` and `perimeter` work in expressions and relations. For a point with center-relative coordinates `(dx, dy)`, set `qx = abs(dx) - (w/2-r)` and `qy = abs(dy) - (h/2-r)`. The signed boundary distance is `hypot(max(qx,0), max(qy,0)) + min(max(qx,qy),0) - r`. `on(P, rounded_rect)` uses this length residual; `inside` requires its negative to be strictly positive under the assertion tolerance. General intersections and tangencies with rounded rectangles are not supported.

SVG renders a native rounded rectangle; PNG follows SVG, and TikZ uses the same corner radius. Layout collision tests use the four straight sides and four circular corner arcs.

`rect` and `diamond` exist because box-and-arrow diagrams drowned in corner plumbing without them. Their anchors (§5.8) are ordinary derived points, legal in constructors and relations.

### 5.6 Connectors and annotations

```
label(text)                        -- standalone measured content; layout owns placement
label(anchor, text)                -- point/anchor attachment
label(path, text)                  -- segment/arrow attachment; placement follows the resolved path
arrow(from_anchor, to_anchor)
tick(s, n)                          -- n equal-length tick marks on segment s, n ∈ 1..4
right_angle(V, P, Q)                -- square mark at V between rays V→P and V→Q
text_block(text)
```

The `text` argument accepts a quoted text string, including any number of inline `$...$` TeX spans, or a bare all-TeX `$...$` literal. See §1.4 and §8.2.

`label(text)` measures text independently and can participate directly in `center`, alignment, containment, ordered spacing, and avoidance. Text width and height come from the renderer, including its typography role, and cannot be independently stretched. No anchor point is synthesized. A standalone label without relations uses deterministic layout defaults; it needs no authored position.

The two-argument `label` overload is selected by its first argument's kind. A point value uses the existing anchor-relative placement. A segment or arrow attaches the label to its resolved centerline with the `near` and `side` preferences in §5.7. Attachment creates no connector or additional geometric point. Several labels may attach to one path, and each is drawn and measured once. A path's layout bounds include its full route and attached labels, excluding external source/target objects, guides, and overlays. These extents participate in collection bounds and layout without making resolved label-box anchors valid inputs to geometric constructors (§5.8).

`arrow(from_anchor, to_anchor)` always retains those endpoint identities, including points constrained to slide along segments. Its route uses the modes in §5.7; automatic routing introduces no authored controls or special loop constructor. Distinct points on the same guide can be connected by an ordinary orthogonal route. Arrowheads and path labels follow the visible routed path, including endpoint clipping (§8.6).

### 5.7 Props per kind

| Kind | Props | Default |
|---|---|---|
| `label` (all modes) | `role: normal \| title` | `normal` |
| `label` (point-anchored) | `dir: auto \| center \| N \| NE \| E \| SE \| S \| SW \| W \| NW`; `gap: expr` | `auto`; style-derived gap |
| `label` (path-attached) | `near: auto \| start \| middle \| end`; `side: auto \| left \| right \| above \| below` | `auto`; `auto` |
| `segment` | `head: -> \| <-> \| -` | `-` |
| `arrow` | `head: -> \| <-> \| -`; `route: direct \| curved \| orthogonal`; `bend: angle` (direct only); `side: auto \| left \| right \| above \| below` (automatic routes only) | `->`; `direct`; `0deg`; `auto` |
| `angle_arc` | `n: 1..4` (multiplicity marks, for distinguishing equal-angle families) | `1` |
| `right_angle` | `size: expr` | style-derived |
| `text_block` | `place: below \| above \| left \| right` (relative to figure bbox) or `anchor: <anchor>, dir: <dir>` | `below` |
| `point` | `mark: dot \| cross \| none` | `dot` |

`dir: center` places a point-anchored label centered on its anchor with zero gap. A standalone label is centered on another object by a `center` relation; it does not accept `dir`, `gap`, `near`, or `side`. A path-attached label accepts `near` and `side`, not point-anchor `dir` or `gap`. Properties used with the wrong label overload are source-anchored errors.

`role: title` selects the preset's title font and size before text is measured. It does not imply centering, top-of-figure placement, or a link to `meta title`; ordinary layout relations place the label. Its explicit or inherited style class still supplies color and other applicable appearance values.

For path labels, `near` prefers the source, middle, or target portion of the visible route. These are preferred regions, not authored numeric fractions; the engine chooses a collision-free position consistent with the preference. `side` is page-relative: left/right mean smaller/larger page x, and above/below mean up/down on the page, independent of traversal direction. Thus `near: start, side: left` can label a downward connector near its source, while `near: start, side: above` labels a rightward connector above it. Placement conflicts are diagnosed when the selected side and available route cannot accommodate the measured label and clearance.

`route: direct` preserves the existing endpoint-to-endpoint line or numeric-bend quadratic. `route: curved` requests a smooth connector whose controls and boundary approach directions are engine-selected. `route: orthogonal` requests axis-aligned segments with right-angle bends; the engine chooses detour distances and corners. For either automatic mode, arrow `side` chooses a page-relative detour side, with `auto` leaving the choice to the engine. An explicit `bend` together with an automatic route, or an explicit arrow `side` with a direct route, is an error. Numerical routing parameters, candidate order, and tie-breaks must be fixed before freezing (Part 12).

Labels do not draw a connector from the anchor to the text box by default. The `gap` property controls spacing only; use an explicit `arrow` for a visible connection.

Arrow bends use a quadratic control point at the chord midpoint plus
`0.5 * tan(bend)` times the perpendicular chord. Bends must satisfy
`|tan(bend)| <= 1000`; singular and near-singular angles (within approximately
0.0573° of an odd multiple of 90°) fail with `E210` at the bend expression,
including during checking of solved scenes. Use a gentler bend such as `45deg`.
Layout and export also reject non-finite control geometry or a control point
more than 500 chord lengths from the midpoint (allowing floating-point rounding).

The bend formula applies only to direct routes. Automatic routes retain source and target anchors, honor explicit clearance requirements, and report a routing conflict when no permitted route is found (§8.6).

### 5.8 Anchors

Anywhere a point is accepted, an **anchor expression** is accepted:

```
A                       -- any point id
s.midpoint  s.start  s.end
c.center    c.at(30deg)
e.center    e.at(30deg)   e.focus1   e.focus2              -- ellipse (§5.10)
e.major_start  e.major_end  e.minor_start  e.minor_end
e.semimajor  e.semiminor  e.rotation                       -- length, length, angle
curve.start  curve.end  curve.at(0.5)                     -- B-spline; scalar parameter (§5.11)
poly.centroid
r.center  r.n  r.s  r.e  r.w  r.ne  r.nw  r.se  r.sw     -- rect / rounded_rect / diamond
r.width   r.height                                        -- numeric anchors
r.corner_radius                                          -- rounded_rect only, length
path.length                                             -- arrow centerline length, before clipping
lbl.center              -- resolved label box center (annotation layer ONLY)
```

Point-valued geometry anchors are ordinary derived points, legal in constructors, `rel`, and `assert`. Numeric properties retain their length/angle types; an arrow's `.length` is a layout-dependent measurement (§6.2). Label-box anchors resolve during layout (§8) and are legal only in annotations, never as inputs to geometric constructors or predicates. Measured label objects can participate directly in the layout relations in Part 3, including content-dependent shape sizing.

`edge(shape, direction)` derives a finite closed boundary segment of an axis-aligned `rect`. The direction is one of the symbolic literals `N`, `E`, `S`, or `W`; it is not an angle or a numbered edge index. Directed endpoints are `.nw → .ne`, `.ne → .se`, `.se → .sw`, and `.sw → .nw`, respectively. The segment follows the shape's resolved dimensions and center, introduces no unknowns or visible drawing by itself, and is legal wherever a segment value is accepted. Endpoints count for `on`. Other shape kinds, diagonal directions, and collections do not have this overload.

```locus
point left() { mark: none }
point right() { mark: none }
rel on(left, edge(frame, W))
rel on(right, edge(frame, E))
rel align_center_y(left, right)
segment divider(left, right) [auxiliary]
```

Here `frame` is an earlier rectangle; points can slide on its finite west/east sides to define a divider without authored positions. This boundary-derived segment is distinct from a box's side-center anchor such as `.w`, which is a single point.

### 5.9 Domains and degeneracy

Every parsed number, evaluated expression, hint, solved value, and resolved coordinate must be finite. Overflow, NaN, or infinity is an error at the source expression that produced it.

- Rounded-rectangle radii must satisfy `0 <= r <= min(w, h)/2`; width and height must be strictly positive.
- Circle radii, rectangle/diamond widths and heights, and `angle_arc` radii must be strictly positive. Label gaps are nonnegative. A `right_angle` size, whether explicit or style-derived, is strictly positive.
- Automatic boxes must resolve to positive dimensions and a valid corner radius. Text values and selected typography roles must have finite measured extents. A routed arrow needs distinct endpoints; a loop uses distinct points on the same object, not coincident endpoints. Invalid role/route/placement tokens, incompatible properties, and unsupported `edge` combinations are source-anchored errors.
- Ellipse semiaxes satisfy `a >= b > 0`; both are lengths. Rotation is a finite angle. Equal semiaxes are valid, with coincident foci, but the value retains its ellipse type and parameterization.
- A B-spline requires at least four finite point-valued controls, whether independently placed or derived. Repeated controls are allowed; the first and last must be distinct. Parameters are finite scalars in `[0, 1]`, without wrapping, extrapolation, or clamping of out-of-domain input. Self-intersection and vanishing derivatives are allowed for evaluation and rendering; no arbitrary tangent is supplied at a stationary point.
- A free `num` has no universal sign restriction; its uses impose constructor-specific domains. A trial vector that violates a domain is rejected as described in §7.1, while a final violation is a construction error.
- A line or ray requires distinct defining points. A zero-length segment may be stored and rendered as a point, but any operation requiring its direction—including `parallel`, `perpendicular`, `foot`, and line-like incidence—reports degeneracy.
- A polygon requires at least three distinct vertices, nonzero signed area, no crossing non-adjacent edges, and no adjacent collinear overlap; it may be clockwise or counterclockwise. This is the “simple polygon” required by `inside`.
- `arc(c, P, Q, ccw)` requires distinct endpoints on `c`. Endpoint incidence is checked with normalized residual `< 1e-12`; `ccw` selects the directed sweep from `P` to `Q`. A full circle must be represented as `circle`, not as an arc with coincident endpoints.
- `angle_arc(A, B, C, r)` and `right_angle(V, P, Q)` require two nonzero rays at their vertex. `tick(s, n)` requires a nonzero segment and an integer `n` in `1..4`.
- Division requires a nonzero denominator under the normalized `1e-12` construction threshold. `sqrt` requires a nonnegative operand; an operand within normalized `1e-12` below zero is clamped to zero, while a more negative operand is an error.

Degenerate inputs never acquire arbitrary fallback directions, centers, branches, or truth values. They produce the source-anchored diagnostics required by §7.5.

### 5.10 `ellipse`

```
ellipse(a, b)                       -- placed; semimajor a, semiminor b, rotation 0deg
ellipse(a, b, theta)                -- placed; major axis at CCW angle theta from +x
ellipse(O, a, b)                    -- derived at existing anchor O, rotation 0deg
ellipse(O, a, b, theta)             -- derived at existing anchor O, rotated
```

The two numeric dimensions are **semiaxes**, not full width and height. Overloads are
resolved by the first argument's type: numeric for a placed ellipse, point for a derived
ellipse. Placed forms are declaration-only (§1.4). A numeric parameter may control a
semiaxis or rotation; its unknown is owned by its `num` declaration.

For internal evaluation, let `U = (cos(theta), sin(theta))` and
`V = (−sin(theta), cos(theta))`. Then `e.at(phi)` is
`O + a cos(phi) U + b sin(phi) V`. Here `phi` is an angle parameter around the ellipse,
**not the polar angle of the ray from its center**, except when `a = b`.
For incidence, `u = dot(P−O, U)` and `v = dot(P−O, V)` (§3.2).

The `.major_start` / `.major_end` anchors are `O − aU` / `O + aU`;
`.minor_start` / `.minor_end` are `O − bV` / `O + bV`. With
`f = sqrt((a−b)(a+b))`, `.focus1` / `.focus2` are `O − fU` / `O + fU`.
Numeric anchors `.semimajor`, `.semiminor`, and `.rotation` return the constructor values.
`area(e)` is `πab`, with `π` the binary64 constant with bits `0x400921FB54442D18`. An ellipse has no
`.radius`, compass-box anchors, or implicit conversion to a circle.

Ellipse anchors, `on`, `inside`, and area measurements are supported. General ellipse
intersection, nearest-point projection, perimeter, ellipse arcs, and tangency overloads
are outside this version; unsupported combinations are source-anchored type errors.
In particular, existing circle-only `arc`, `project`, `tangent`, and `tangent_line`
constructors/predicates do not silently accept an ellipse.

### 5.11 `bspline`

```
bspline(P0, P1, P2, P3, ...)        -- derived open, clamped, uniform cubic B-spline
```

Control points are ordered point expressions, with at least four arguments. Named
`point()` declarations are valid controls, as are anchors and derived constructions. Their order
defines the traversal from `.start` (first control) to `.end` (last control). Interior
controls guide the curve and are **not generally interpolated**. There is no control-point
coordinate list, free-control syntax, or automatically created control polygon. Declare
segments between controls when a construction polygon should be shown.

The degree is fixed at `p = 3`; all weights are one. For `m` controls `P0 … P(m−1)`, the
knot vector has `m+4` entries: four zeros, the simple interior knots
`j/(m−3)` for `j = 1 … m−4`, then four ones. Thus four controls produce one cubic Bézier
segment; five or more produce multiple polynomial spans. Interior simple knots give
`C²` continuity. These are standard clamped B-spline conventions
([Open CASCADE reference](https://dev.opencascade.org/doc/occt-7.7.0/refman/html/class_geom2d___b_spline_curve.html)); the fixed degree and automatic knots are Locus's initial authoring contract.

`.at(t)` takes a **dimensionless scalar** in `[0, 1]`, not an angle or an arc-length
fraction. `.start` is exactly `.at(0)` and `.end` is exactly `.at(1)`. The spline has no
center, radius, area, or length measurement in this version. Numeric comparisons of
distances between sampled points remain available.

Evaluation uses de Boor's recurrence. Return the first or last control exactly at `t = 0`
or `1`. Otherwise choose the unique span `k` with `u[k] <= t < u[k+1]`, initialize
`d[j] = P[k−3+j]` for `j = 0 … 3`, and for `r = 1 … 3`, visit `j = 3 … r` in descending
order. Set `i = k−3+j`, `alpha = (t−u[i]) / (u[i+4−r]−u[i])`, and replace
`d[j]` with `(1−alpha)d[j−1] + alpha d[j]`. Return `d[3]`. Every denominator in these
active spans is positive under the specified knot vector. An exact interior knot uses
the span on its right; automatic differentiation follows the same evaluation. Although
endpoint values are returned exactly, their derivatives with respect to `t` are the
one-sided curve derivatives, not zero: `C'(0) = 3(m−3)(P1−P0)` and
`C'(1) = 3(m−3)(P(m−1)−P(m−2))`. Derivatives with respect to control points still follow
the endpoint identity. Implementations must preserve both contributions when `t` and
controls depend on solver unknowns.

Point samples and endpoint relations compose with the existing language. General
curve incidence, intersections, projection, tangency, custom degree or knots, weights
(NURBS), periodic/closed splines, fitting, and interpolation-through-points constructors
are outside this version. Unsupported operations produce type errors rather than
silently sampling the curve or treating its control polygon as the curve.

---

## Part 6 — Expression language

### 6.1 Numeric dimensions and inference

Every numeric expression has one of four dimensions: **scalar**, **length**, **area**, or **angle**. Length is measured in the file's arbitrary scene unit; area is its square. Angles are stored internally in radians, with `deg` and `rad` accepted at the source boundary. Dimensions are semantic types, not user-selectable unit systems.

- Geometry determines dimensions: `dist`, `hdist`, `vdist`, `radius`, `length`, arrow `.length`, `style_metric`, box dimensions, ellipse semiaxes, and relative point distances are lengths; `area` is area; `angle`, `dir_angle`, ellipse rotation and ellipse `.at` arguments, and direct-arrow `bend` are angles; B-spline `.at` arguments, counts, and ratios are scalars. Typography roles, route modes, label preferences, and boundary directions are symbolic choices, not numeric dimensions.
- A numeric literal without `deg` or `rad` begins as a dimension variable and is unified from context. Thus the `6` in `dist(A, B) == 6` becomes a length, while the `2` in `2 * angle(A, B, C)` is scalar. An otherwise unconstrained literal defaults to scalar.
- Each named `num`, including a free `num`, has one dimension inferred from all of its uses in the file. Conflicting uses are a type error. A free numeric hint has the inferred dimension of its declaration.
- Addition, subtraction, comparison, `min`, and `max` require matching dimensions. Multiplication or division by a scalar preserves the other operand's dimension. `length * length` produces area; `area / length` produces length; equal-dimension division produces scalar. Other dimension products are errors in `0 beta 6`.
- `abs` preserves dimension. `sqrt` accepts a scalar or an area and returns a scalar or length respectively. `ratio(a, b)` requires equal dimensions and returns scalar.
- Constructor arguments and properties impose their documented dimensions. In particular, circle radii, rectangle/diamond sizes, label gaps, tick/right-angle sizes, and signed point distances are lengths. A bare number in an angle position remains an error; contextual inference never turns it into an angle without `deg` or `rad`.

Dimension inference is whole-file and deterministic even though value references remain definition-before-use. The JSON AST reports the inferred dimension for every numeric declaration and expression so agents can localize type errors.

### 6.2 Numeric functions

| Function | Result |
|---|---|
| `dist(P, Q)` | Euclidean distance |
| `hdist(P, Q)` / `vdist(P, Q)` | signed axis offsets: `x(Q)−x(P)` / `y(Q)−y(P)` |
| `angle(A, B, C)` | unsigned interior angle at `B`, in `(0deg, 180deg)` |
| `dir_angle(A, B)` | CCW angle of vector A→B from +x, in `[0deg, 360deg)` |
| `ratio(a, b)` | `a / b` (sugar — agents state ratios constantly) |
| `radius(c)` `length(s)` `area(poly)` `perimeter(poly)` | as named |
| `area(e)` | ellipse area `π × e.semimajor × e.semiminor` |
| `style_metric(name)` | preset-defined positive length; supported names are `"short_length"` and `"narrow_width"` |
| `length(path)` / `path.length` | arrow's untrimmed centerline length, excluding arrowheads and labels |
| `sqrt(x)` `abs(x)` `min(a, b)` `max(a, b)` | as named |

`style_metric(name)` requires a plain string naming a supported metric. `short_length` supplies a general short-path length, and `narrow_width` supplies a general narrow-shape width; neither is tied to sequence-diagram meaning. Values are scene-unit lengths supplied by the active preset, independent of solved positions and bounding boxes. Unknown names and non-string arguments are errors. Exact values and the preset's scene/text scale must be pinned before freezing; metrics may not be looked up from platform defaults or inferred from the output viewport. An appearance class does not redefine these metrics.

An arrow's `.length` and `length(path)` are the same measurement. For a straight route this is endpoint distance; for an orthogonal route it is the sum of span lengths; for a curved or numeric-bend route it is centerline arc length. Clipping changes visible ends but not this logical measurement or endpoint identities. This does not add a general B-spline length or ellipse perimeter overload. The deterministic curved-path arc-length procedure and tolerances remain to be pinned (Part 12).

```locus
rel sample.length == style_metric("short_length")
rel overlay.width == style_metric("narrow_width")
```

Numeric measurements are legal in `rel` equalities and `assert` comparisons. Geometric functions differentiate through their definitions under §7.1. Measurements of automatic routes participate in the joint layout dependencies (§8.6); changing discrete route candidates is not assumed to be a smooth geometric function.

### 6.3 Predicate summary

The unified vocabulary and mode restrictions are in §3.1–§3.5. Equality-based geometric relations work in `rel` and `assert`; `between`, `inside`, and `< > <= >=` remain assert-only under the current equality solver. Measured layout requirements need the additional contract in Part 12, and `trim_endpoints` is a standalone rendering operation. Angle/length equality uses `==` on numeric functions rather than dedicated predicates: `angle(A, B, C) == 90deg`.

---

## Part 7 — Solve and evaluation semantics (normative)

### 7.1 The solve

The geometric equality solve contract is defined below. Measured layout adds the registry, initializer, hard inequalities, working-set preferences, and bounded feedback rules in §7.7 and §8.7. Implementations may optimize the frozen solver only if they produce the same resolved `f64` values and reports for every valid input; changing any frozen constant or acceptance rule requires a new format version. The beta 6 layout correction in §7.7 explicitly supersedes the earlier ordering-before-alignment seed schedule and full-step-only feasibility restoration. This correction may change measured-layout coordinates and iteration reports; it preserves geometric constraints, exact containment, and the pure-geometry solve contract. The corrected schedule is frozen below.

1. **Unknown vector and initialization.** Concatenate each `point()` declaration's internal `(x, y)`, each placed shape's internal center `(x, y)`, and each free numeric value in declaration order (§2.2). Build the initial vector `h` from engine-generated point and center placements and authored numeric hints. Initialization must account for shape dimensions and relation dependencies, avoid coincident-point or coincident-center degeneracy where a direction is needed, and use a bounded deterministic retry schedule when an initial arrangement is invalid. It must accept a standalone `point()` or `circle(2)` declaration without any additional input. The initializer may not read asserts to choose a solution or change explicit branch semantics. Normalize unknown coordinates before linear algebra: point/center coordinates and length-valued nums by `D`, area-valued nums by `D²`, angle-valued nums by `1 rad`, and scalar nums by `max(1, abs(hᵢ))`, where `D` is the solve-time characteristic length from §7.4. The normalized vector is `z`; its normalized initial vector is `zₕ`. All point/center components and placement seeds remain internal.
2. **Residual vector.** Desugar `where` clauses in source order, flatten allowed `and` conjunctions left-to-right, and expand each geometric equality relation into the component residuals specified in §3.3. Evaluate these residuals in that order and normalize each by §7.4. Layout constraint expansion follows §7.7; endpoint clipping contributes no residuals. Tangency type and explicit constructor branch indices come from the source and remain unchanged throughout the solve.
3. **Derived evaluation and differentiation.** For the initial vector and every trial vector, evaluate the transitive dependencies of relations and placed-shape dimensions top-to-bottom as pure functions of the current unknown values before evaluating residuals. Derived objects used only by annotations or asserts are deferred until final evaluation; for example, the intersection of two circles being solved into tangency need not exist at initialization. Compute the Jacobian `J = ∂r/∂z` with forward-mode automatic differentiation through those definitions; finite differences are not conforming. Layout pieces differentiate the active piece of the current evaluation, with first-operand ties. Linear incidence `on(point, line/ray/segment)` differentiates its current distance: finite carriers select the current endpoint or interior piece, and the interior absolute value uses the current sign. This beta 6 convergence correction also applies to pure geometry; freezing the seed piece can otherwise produce a Jacobian inconsistent with the measured distance and reject feasible incidence systems. Other pure-geometric absolute-value, min/max, finite-carrier constructor/tangency and arc pieces retain initialization-selected derivatives for compatibility through a numerical solve. Layout-involved property anchors and incidences continue to use the relation-aware initializer and live finite-carrier AD. These scoped exceptions do not imply live derivatives for all geometric operations. A construction precondition failure at initialization requests the next bounded initialization attempt; a failure at a later trial gives that trial infinite objective without leaking NaN or infinity into output. Explicit branches retain their documented geometric ordering at each evaluation.
4. **Rank and null space.** Compute an SVD of `J` with the deterministic one-sided Jacobi procedure below. A singular value has rank when `σᵢ > 1e-10 × max(σ₀, 1)`. The Moore–Penrose pseudoinverse `J⁺` uses exactly those retained singular values, and the null-space projector is `N = I − J⁺J`. Sign choices for singular vectors do not affect `J⁺` or `N`.
5. **LM trial step.** Start with `λ = 1e-3`. At each attempt, compute the primary step from `(JᵀJ + λI) δₚ = −Jᵀr` using Cholesky factorization in ascending row/column order. Compute the settling component `δₕ = N(zₕ − (z + δₚ))` restricted to geometric unknown columns, then use `δ = δₚ + δₕ / (1 + λ)`. All dot products and sums execute sequentially in index order; contraction into fused multiply-add is forbidden.
6. **Acceptance schedule.** Let `F(z) = 0.5 × Σ rᵢ²`, summed in residual order. Accept a finite trial if its maximum absolute residual is below `1e-12`, or otherwise only if its `F` is strictly smaller. After acceptance set `λ = max(λ / 10, 1e-15)` and recompute values, `J`, rank, and `N`. After rejection keep `z` unchanged and set `λ = min(λ × 10, 1e15)`. An attempt, accepted or rejected, counts as one iteration. Stop successfully when `max(abs(rᵢ)) < 1e-12`; a file with no relations requires zero numerical iterations after valid initialization. End this initialization attempt after 200 numerical attempts or after a rejection at `λ = 1e15`; report solve failure only after the bounded initialization schedule is exhausted.
7. **Lexicographic settling semantics.** Relation satisfaction is the primary objective. Null-space motion toward `zₕ` is secondary and may never justify accepting a worse primary objective. Remaining freedoms settle deterministically toward engine placement defaults and numeric hints; this is not a claim of a global closest solution across disconnected branches. Engine placement does not become an authored constraint.
8. **Final evaluation and asserts.** Evaluate every derived object top-to-bottom at the accepted unknown vector, build the final scene, and then check asserts using §7.4. This final pass uses the same functions as residual evaluation and introduces no new geometry semantics. A failed assert is reported as a failed test, never used to trigger a search for a different arrangement.

All arithmetic is IEEE-754 binary64, round-to-nearest ties-to-even, with subnormals preserved. Transcendental functions come from the format-version-pinned `libm` implementation. No randomness, parallel reductions, locale-sensitive parsing/formatting, reassociation, or platform BLAS/LAPACK calls are permitted in the normative path.

For the one-sided Jacobi SVD, start with `A = J` and `V = I`. A sweep visits every column pair `(p, q)` in lexicographic order. Let `α = dot(Ap, Ap)`, `β = dot(Aq, Aq)`, and `γ = dot(Ap, Aq)`, with dots summed by ascending row. Skip the pair if either norm is zero or `abs(γ) <= 2^-48 × sqrt(αβ)`. Otherwise set `ζ = (β−α)/(2γ)`, `t = 1` when `ζ = 0`, and otherwise `t = sign(ζ)/(abs(ζ)+sqrt(1+ζ²))`; then `c = 1/sqrt(1+t²)` and `s = c×t`. Replace the columns with `(c×Ap−s×Aq, s×Ap+c×Aq)` and apply the same rotation to `V`. Stop after the first sweep that performs no rotations, or fail with numerical non-convergence after 64 sweeps. Singular values are the final column norms; sort them descending with the original column index as the stable tie-break, and derive `U` by normalizing nonzero columns. This procedure, including its skip threshold and ordering, is part of `0 beta 6`.

### 7.2 DOF accounting

`check` and `describe` always report unknown count, equation count, rank, global remaining DOFs (`unknowns − rank`), initialization attempts, numerical attempts, accepted steps, maximum normalized residual, and final damping. Counts may expose how much freedom remains, but never the internal coordinate vector.

For each independently placed point, placed shape, or free numeric parameter, take the rows of the final null-space projector `N` (§7.1) belonging to that object's components. The rank of that row block, using the same singular-value threshold as §7.1, is its **local remaining DOF count**. Zero means fully constrained; a positive value means that many independent global null directions can move the object or change its parameter. Local counts intentionally need not sum to the global remaining count because one global freedom, such as translation, can move several objects. A derived anchor contributes no independent unknowns and is attributed to its upstream owners.

Common translation of the whole geometric scene is always harmless: no language construct refers to an absolute origin. Its two independent directions, when placed points or shapes exist, are included in raw DOF counts but excluded from actionable under-constraint diagnostics and `--strict` failures. A standalone point or fixed-radius circle therefore has two raw placement DOFs, zero unresolved relative DOFs, and passes `check --strict`. Relative movement between disconnected objects is still unresolved; it is not exempted as common translation. Additional rigid symmetries, such as a common rotation in a figure without directional intent, need the classification rules identified in Part 12 before receiving the same exemption.

To compute unresolved relative DOFs, remove the span of the two common-translation vectors from the final null space: each vector moves the matching component of every placed point and every placed shape's center equally and leaves numeric parameters unchanged. Apply the same rank threshold as above. Per-object actionable counts use row blocks of this reduced projector. Reports distinguish these counts from raw local counts; they must not claim that an anchor is independently free.

For a free `num`, report its authored hint and solved value. Emit `hint_far` only when its absolute normalized change exceeds `0.25`, including the threshold and actual change. Never report point or center seed displacement or emit placement-based `hint_far`; internal initialization is not user intent.

### 7.3 Failure taxonomy

| Condition | Detection | Exit | Report |
|---|---|---|---|
| Under-constrained | unresolved relative DOFs remain after removing common translation (§7.2) | 0 + info; `--strict`: 1 + warning | affected points, shapes, or numeric parameters, with suggested geometric relations |
| Over-constrained, consistent | redundant rows, residuals converge | 0 + warning; `--strict`: 1 | the redundant relation(s) by source line |
| Over-constrained, conflicting | inconsistent defining facts established by analysis; otherwise report non-convergence | 2 | best-fit residual per relation, worst first; minimal infeasible subset when isolable |
| Non-convergence / initialization failure | bounded initialization attempts exhausted without a valid converged solve | 2 | affected object ids, construction preconditions or final relation residuals; suggestions involving relations, dimensions, or explicit branches |

Generic-position figures may intentionally leave relative freedoms, hence "info" rather than "warning" by default. Harmless common translation is solver bookkeeping, not a request for additional author input. A failed bounded solve does not by itself prove that the relations are inconsistent.

`--strict` is a `check` policy, not a different solver: it promotes under-constraint information to warnings and returns exit 1 if any warning remains, including a consistent redundant relation. Without `--strict`, warnings are reported but do not change a successful exit code.

```
figure.locus  solve: 4 unknowns, 1 equation, rank 1
  placement: 2 common-translation DOFs handled by Locus
  info[I310]: c1 and c2 have 1 unresolved relative DOF
     12 | circle c2(3)
        = add align_center_y(c1.center, c2.center) if alignment is intended
  solved: max scaled residual 3.1e-13 in 6 numerical iterations
```

### 7.4 Tolerance

All tolerances operate on a **normalized, dimensionless residual**. The default assert/incidence threshold is `ε = 1e-9`. `tol t` replaces `ε` for every atomic comparison or predicate inside that assert; `t` must be a finite positive scalar literal. It is a normalized factor, never an absolute scene-unit distance.

The characteristic length `D` is computed deterministically. During each initialization attempt it is the bounding-box diameter of all placed points and shapes and the geometry in the dependency closure needed for relations and shape dimensions, evaluated at that attempt's initial vector; it stays fixed for that numerical solve. Deferred constructions (§7.1) do not participate in this initial bound. During assertion and incidence checking it is the geometry-only solved bounding-box diameter of the complete scene. Circle/arc extents and polygon/rect/diamond vertices participate whether or not their marks are visible. Ellipses use their exact rotated bounds; B-splines use the conservative bounding box of their control points, independent of render tessellation (§8.5). An unbounded line or ray contributes only its finite defining points, not an infinite carrier; annotations do not participate. If that diameter is zero, use the largest finite positive length-valued numeric hint, literal, or constructor argument in declaration order; if none exists, use `D = 1`.

For numeric comparisons, both operands have the same inferred dimension (§6.1). Their normalization scale `S` is:

| Dimension | `S` |
|---|---|
| scalar | `max(1, abs(lhs), abs(rhs))` |
| length | `max(D, abs(lhs), abs(rhs))` |
| area | `max(D², abs(lhs), abs(rhs))` |
| angle | `1 rad` |

Equality passes when `abs(lhs − rhs) / S <= ε`. Strict comparison uses a tolerance dead band: `lhs < rhs` requires `(rhs − lhs) / S > ε`, while `lhs <= rhs` requires `(lhs − rhs) / S <= ε`; `>` and `>=` are symmetric. Boolean `and`, `or`, and `not` combine the already evaluated atomic truth values.

Each geometric equality predicate defines a raw residual with a dimension in §3.3; it is divided by the matching scale above before comparison with `ε`. Multi-equation predicates pass only when every component passes. Assert-only containment predicates use the boundary rules in §3.3. Detected incidences use the same normalized residuals and default `ε`, so `inspect` and `assert` cannot disagree solely because they use different epsilon systems.

The geometric equality solve criterion is `max(abs(normalized relation residual)) < 1e-12`, strictly tighter than the default assert threshold. A converged geometric equality relation therefore cannot fail when restated as a default-tolerance `assert` with the same branch and domain semantics. Layout checks require the measurement and tolerance contract identified in Part 12; `trim_endpoints` has no Boolean assertion form.

### 7.5 Construction errors

Derived-tier failures — empty `intersect`, `circumcircle` of collinear points, `tangent_line` from an interior point, division by zero — abort with **exit code 2** and a source-anchored diagnostic:

```
figure.locus:9:11  error[E201]: intersect(c1, c2): circles do not intersect
                   c1 radius 1.20, c2 radius 0.80, center distance 3.41 > 2.00
   9 | point P = intersect(c1, c2)
```

Diagnostics must name the op id, the failing precondition, and the offending measured values. No NaN or infinity ever leaks into a resolved scene.

### 7.6 Determinism (normative)

Identical file bytes → identical solve → identical internal coordinates → byte-identical SVG. All tie-breaks (initialization and retry schedule, LM schedule, branch ordering, label candidate selection, float formatting) must be frozen without randomness. The initialization rules in §7.7 are part of geometric equality conformance. `locus-core` must use a pinned math library (`libm` crate) so this holds across platforms. This enables golden-file snapshot testing in CI, including solve-heavy fixtures on every platform in the matrix. Rendered formats necessarily contain coordinates; those are generated output, not an authoring or agent-feedback interface.

---

### 7.7 Registry, initialization, and measured preference solve (normative)

Build the complete unknown registry in expanded declaration order before selecting dependencies. Unreferenced placed objects and free numeric parameters retain their unknowns. Automatic box centers are geometric; their width/height are automatic-dimension unknowns. Standalone-label centers are label-placement unknowns. Orthogonal detours, curved control offsets and path-label `t`/`o` are internal route/label-parameter unknowns. Engine classes do not create actionable under-constraint warnings when preset defaults apply and never appear as free authored points or nums. Common translation includes all placed centers.

The solver dependency closure includes defining relations, collection members, route endpoints, and registry-member dimension/argument expressions. Derived declarations outside it are deferred until final evaluation. Solve-time `D` includes only the registry and closure. This can change historic pure-geometry rounding when deferred geometry formerly enlarged `D`; the largest observed corpus distance change is about `1.21236e-13`. Ring seeds and historical frozen geometric pieces remain unchanged. Deferred bounds must not be reintroduced merely to preserve old output bits.

Pure geometry uses successive seed pairs `(0,0), (4,0), (0,3), (4,3), (-4,0), (0,-3), (-4,3), (4,-3)`. For declaration index `i`, add `(1.25r,-0.75r)` where `r = floor(i/8)`. These are internal implementation coordinates. Initialization-precondition retries use placement scales `1, 0.25, 0.5, 2, 4, 0.125, 8, 16` in that order; type, property, dependency and authored branch errors do not trigger retries.

For layout, first seed automatic dimensions at the maximum of preset size and measured content plus padding, then seed property anchors and numeric dimensions/lengths. Seed shared coordinates on x, then y before building ordered graphs: alignment must remove arbitrary ring-seed offsets from composed row/column bounds before those bounds determine spacing and container dimensions. Build per-axis ordered graphs from separation, inset, cover and containment requirements after collection expansion, with alignment components sharing coordinates. Run two graph/aggregate passes on x, then two on y; DAG longest paths use full extents and preferred gaps, while cyclic components receive bounded deterministic component offsets. Seed containers, refresh translated derived bounds, then shared coordinates, finite-carrier incidences and measured guide-caption spacing. Preserve ordered container axes. Standalone labels use a center partner when present and ring placement otherwise. Re-evaluate primal values and `D` after layout seeding and before AD branch validation. Asserts never select seeds.

Hard residuals are geometric/layout equalities and `min(g,0)/S` for each `g >= 0` requirement. At `g = 0` the hinge retains its gradient; a strictly positive row has zero gradient. Reject nonfinite normalized rows before selecting hinge pieces. Secondary preferences use unit weight per normalized row: authored numeric parameters toward their hints, automatic dimensions toward preset dimensions, label/route parameters toward seeded defaults, and layout margins/separations toward their preset minimum. In measured layout systems, engine-generated point, shape-center and standalone-label placement seeds only initialize the solve; they do not contribute competing positional preferences. Pure geometry retains its initialized-default settling semantics. Hard authored dimensions, branches and relations take priority; a required larger gap remains valid. Phase 1 keeps the §7.1 LM schedule and the aggregate gap, cover and signed-distance containment rows of §8.7, preserving their residual weights. Phase 2 uses the equivalent individual hard faces specified there. Splitting hard faces never duplicates compact-gap preferences or alignment equalities.


A primal active-set iteration over the feasible set `{r_eq = 0, g ≥ 0}` that minimizes the
merit function `Φ(z) = 0.5 Σ qᵢ²` and never leaves the last accepted feasible iterate without
improving it. Notation: `z` is the normalized unknown vector of §7.1 step 1; each `qᵢ` is
normalized by the same scale `S` as a residual of its dimension, so `Φ` is dimensionless;
`Q = ∂q/∂z` (forward-mode AD like `J`); `N_W = I − J_W⁺J_W` is the null-space projector of
the working set, computed with the §7.1 Jacobi SVD and rank threshold; `∇Φ = Qᵀq`.
For layout settling, form this projector directly as the sum of outer products of the
discarded right singular vectors, in descending singular-value order with the same index
tie-breaks. This avoids cancellation in the inverse-product formula near dependent active
constraints. The legacy geometric projection keeps its established arithmetic order.

1. Initially nominate inequalities with `g ≤ τ_active` (`1e-7`). Before every direction,
   retain only genuinely binding nominations (`g < 1e-12`): a positive slack between
   hard-feasibility tolerance and activation tolerance must not become an equality or
   certify interior stationarity. Slack nominations still participate in blocking.
   Retain an independent boundary basis in ascending row order, modulo the equality row
   space. Project each nominated row through the equality null projector, then perform
   two ordered modified Gram–Schmidt passes against retained unit rows. Retain a row
   when its remaining Euclidean norm exceeds `1e-10 × max(1, ‖row‖₂)`.
   `W` contains all equalities and this boundary basis. Removed dependent inequalities
   remain in every hard-feasibility check and blocking test; they can block subsequent
   motion. They are never discarded from the problem.
2. **Direction**: the projected Gauss–Newton direction `δ = −N_W (Q N_W)⁺ q`, with the
   pseudoinverse from the same SVD procedure, together with the multipliers of the rows in
   `W`. Bound the normalized step's Euclidean norm by the preset trust radius `8`. First
   try the undamped step; if it exceeds the radius, replace each retained inverse-square
   weight `1/σ²` by `1/(σ² + λ)`. Start `λ` at `1e-6` times the largest squared singular
   value, multiplying by `10` until a fitting upper bracket exists, then bisect the
   damping bracket to reach the trust boundary within relative squared-norm tolerance
   `1e-6`, with at most `64` trials. Return the last fitting step if the trial cap is
   reached. This avoids excessive damping and spurious release/block cycles. Discarded
   singular directions remain discarded. Trust damping never changes hard constraints,
   feasibility acceptance or the projected-gradient convergence test.
   The undamped directional derivative is `∇Φ·δ = −‖Π q‖² ≤ 0`, where `Π` is the orthogonal
   projector onto the **retained singular subspace** of `Q N_W` — the span of the left
   singular vectors whose singular values pass the rank threshold. With truncation this is a
   proper subspace of `range(Q N_W)` (the `diag(1e11, 1)` example in step 4 retains only the
   first direction), so `δ` is a descent direction whenever it is nonzero, but a zero `δ` does
   not imply a zero projected gradient. Damping multiplies each retained squared-residual
   term in the slope by `σ²/(σ² + λ)`, preserving a nonpositive directional derivative.
3. **Release**: compute multipliers from `J_Wᵀ μ = −Qᵀ(q + Qδ)` by the pinned
   least-squares procedure. When the projected-gradient convergence threshold is
   satisfied, instead use the **current** gradient, `J_Wᵀ μ = −Qᵀq`: a large model
   step in a weakly weighted tangent direction must not hide a required release at
   the current iterate. Inequality multipliers are unique modulo the equality row
   space because the boundary directions are independent. A sign-valid certificate has
   `μ_ineq ≤ τ_stop`; excluded dependent rows receive zero multipliers. An unrestricted
   minimum-norm solution over all dependent inequalities is not a sign certificate.
   If any inequality in `W` has `μ > τ_stop`, indicating the preference direction
   would move into its interior, remove the one with the lowest row index and return to
   step 2. This lets `x ≥ 0` release when the preference is `x = 1`.
4. **Convergence test**: compute the projected gradient `‖N_W ∇Φ‖∞`. If it is
   `≤ τ_stop · max(1, ‖q‖∞)` (`τ_stop = 1e-9`, normalized coordinates) and no release is pending,
   settling is **complete**: stop with `settling_complete: true`. This is the only exit that
   may report completion.
   **Stall guard**: otherwise, if `(N_W ∇Φ)·δ ≥ −τ_dir ‖δ‖₂` (`τ_dir = 1e-14`) — a negligible or
   non-descent slope per unit direction while the projected gradient is still large — the iteration has **stalled**
   numerically, not converged: keep the current feasible iterate and stop with
   `settling_complete: false`. The direction is never line-searched. Example: with `N_W = I`,
   `Q = diag(1e11, 1)`, `q = (0, −1)`, the §7.1 rank threshold `1e-10 × max(σ₀, 1) = 10`
   discards the second singular direction, so `δ = 0` and the slope is `0` although
   `‖N_W ∇Φ‖∞ = 1`; this must be reported as a stall. Evaluate the slope using the
   tangent gradient to avoid cancellation against constraint-normal components.
5. **Block**: ratio-test `δ` against all inactive inequalities; truncate by the factor
   `ρ ∈ (0, 1]` to the first blocking boundary (lowest row index on ties) and mark that row for
   addition to `W`. Ignore predicted rates `Aᵢδ ≥ −1e-14 × max(1, ‖Aᵢ‖₂) × max(1, ‖δ‖₂)`
   in this ratio test to avoid zero-distance pivots caused by projection roundoff.
   Every trial still checks all hard inequalities and restores any actual violation.
   Positive scaling preserves descent.
6. **Line search**: for `α ∈ {1, ½, ¼, …}` down to `α_min = 1/1024`, form the trial
   `z + αρδ`, run **feasibility restoration** (at most `k_restore = 8` damped LM corrections with `W`
   fixed), and accept the first trial that is feasible (`max|r| < 1e-12`) and satisfies the
   Armijo condition on the directional derivative
   `Φ(trial) ≤ Φ(z) + c·α·∇Φ(z)·(ρδ)` with `c ∈ (0, ½)` preset (`1e-4`).
   Because the right-hand side uses the actual slope, an exact Gauss–Newton step toward a
   solution always passes: for `q = 0.001(x − 1)` from `x = 0`, `δ = 1`, `∇Φ·δ = −1e-6`, and
   `Φ(1) = 0 ≤ 0.5e-6 − 1e-10`. Full steps without the test cycle on cases such as
   `q = (x, 2x³ − 1.5x² − 2x + 2.5)` (0 → 1 → 0, `Φ` 3.125 → 1 → 3.125); the second step
   fails the decrease condition and is backtracked.
   Restoration's linear solve contains all equality rows, the independent active
   boundary rows, and inactive inequalities that actually violate hard tolerance
   (`g ≤ −1e-12`). Feasible inactive faces, including zero-valued redundant faces,
   do not become equality penalties. Every restoration trial still checks every
   hard row; a newly violated inactive face enters the next correction. This avoids
   both spurious pinning and quadratic zero-row weighting after bound expansion.
   Each LM correction uses its own bounded backtracking search: try factors
   `β ∈ {1, ½, ¼}` in that order. Re-evaluate every hard row at each trial,
   including previously feasible inactive inequalities. Accept the first finite trial
   that is working-feasible or strictly decreases `0.5 Σ r_restore²`, rebuilding that
   merit's row set at the trial. An invalid evaluation rejects only that factor.
   Start correction damping at `1e-12`; after an accepted correction divide it by ten
   with a floor of `1e-15`. If all factors fail, retain the current restoration iterate
   and multiply damping by ten. The eight-correction cap counts linear correction
   solves, including rejected ones; all factor evaluations count in `evaluations`,
   while `line_search_evaluations` counts only the outer preference trials. Correction
   merit reduction alone never authorizes an outer move: working feasibility and the
   preference Armijo condition above still apply.
7. On acceptance: commit `z`, add any blocking row to `W`, re-evaluate `r`, `q`, `Q`, and `W`,
   and double the current trust radius up to its preset maximum. On rejection at `α_min`,
   or when restoration exhausts `k_restore` for every `α`, keep the last accepted iterate,
   contract the trust radius by `0.25`, and recompute the direction. Stop with
   `line_search_failed` if the next radius would be below `1e-6`. Radius retries count
   against the existing total direction-iteration cap; they never count as accepted moves.
8. Return to step 1. Allow at most 256 accepted preference moves, 64 consecutive
   working-set pivots without an accepted move, and 1024 total direction iterations.
   A release or zero-distance block is a pivot, not an accepted move. After the last
   allowed move, the convergence test may still certify completion before another move.
   Exhausting any cap ends the loop with `settling_complete: false`; completion is
   decided solely by the convergence test in step 4.

Lowest-index tie-breaking makes release/block choices deterministic; it is not relied on to
prevent cycling — the Armijo decrease on `Φ` is.

**Bounded curvature recovery.** After at least one accepted move, permit one model-rebuild
attempt per continuous solve when no release is pending, the ordinary direction has
negative slope, and the projected gradient lies above the convergence threshold but at
or below `1e-5 × max(1, ‖q‖∞)`. The Gauss–Newton model omits curvature of preferences
and restored hard boundaries; it can otherwise converge slowly and lose resolvable
objective decrease before stationarity. This recovery changes neither feasibility nor
the stationarity threshold.

Build an orthonormal tangent basis by visiting projector columns in order, applying the
same two orthogonalization passes and retaining norms above `1e-10`. Attempt recovery
only for 1–32 retained directions. For each direction, restore probes at `z ± 1e-4 v`
with the same active basis and eight-attempt restoration cap. Reject the rebuild if a
probe fails or its restoration correction exceeds `0.1 × 1e-4` in Euclidean norm.
At each probe recompute the working projector and tangent preference gradient. Central
differences of these gradients, projected into the original tangent basis, form a reduced
Hessian; symmetrize it by averaging transposed entries. Solve its Newton equation by
the pinned least-squares procedure. Retain this direction only if its tangent-gradient
slope is negative and its infinity norm is at most eight times the ordinary direction's.
An unavailable or rejected rebuild leaves the original direction in place. A retained
direction uses the same blocking, feasibility restoration, Armijo test and rollback as
every ordinary move. Probe evaluations never commit geometry, and completion still
requires step 4. Reports count model-rebuild attempts separately; restoration counts
include probe restoration work.

**Exit outcomes.** Settling always terminates on a feasible iterate: the last accepted one.
`settling_complete: true` is reported only when the projected-gradient test in step 4 passes.
Every other exit — stall guard, rollback after a failed line search or restoration, or the
step cap — leaves the figure valid and solved but reports `settling_complete: false` together
with the exit reason (`stalled`, `line_search_failed`, `step_cap`), the remaining
projected-gradient norm, and the relations in `W`; `check --strict` treats it as a warning,
not an error. Restoration failure inside the line search only rejects a trial and is never
surfaced as a solve failure.

**Conditioning.** Both working-set and preference pseudoinverses retain the §7.1 rank threshold. No smaller preference cutoff is adopted. Normalized preferences can still be ill conditioned; an incomplete result must stay incomplete in reports. Total settling iterations, accepted preference moves, working-set pivots, model-rebuild attempts, line-search evaluations, total live state evaluations (including probes), and restoration steps are separate counters.

Equality rank uses equality rows only. Reports expose active requirement source spans and remaining active directions separately; an active inequality bounds motion on one side and must not be represented as an authored equality.

## Part 8 — Annotation layout and rendering

### 8.1 Label placement

For each point-anchored label with `dir: auto`:

1. **Candidates**: 8 compass directions × 2 gap radii around the anchor, plus domain-informed preferences — vertex labels prefer the external angle bisector direction; segment labels prefer the two perpendicular offsets; circle labels prefer outward along the radius.
2. **Scoring**: penalties for overlapping strokes, other label boxes, or exceeding figure margin.
3. **Assignment**: greedy, most-constrained-first, deterministic tie-break (candidate list order). Any label left with a positive collision score is reported as a diagnostic — never silently accepted.

Pinned `dir` (including `center`) bypasses the heuristic entirely.

Standalone labels are measured content placed by layout relations and deterministic defaults (§5.6); they do not use point-anchor direction candidates. Path-attached labels use the resolved visible route, prefer the region named by `near`, and remain on the page side named by `side`. The engine chooses offsets with style clearance, accounts for strokes, shapes, and other labels, and uses deterministic candidate ordering. An unsatisfied explicit avoidance requirement is a layout conflict; ordinary annotation collisions remain source-anchored diagnostics.

### 8.2 Mixed text and TeX

Each text value is laid out as its ordered segment list (§1.4). Plain segments use the preset's deterministic text font and metrics; TeX segments render server-side using the guaranteed TeX subset. Adjacent segments share one baseline and are concatenated without implicit whitespace. A `\n` in a plain segment ends the current line and starts a centered new line; TeX spans cannot contain raw line breaks.

The layout engine resolves the label's typography role first, measures every segment with its actual renderer, composes the line boxes, and feeds the resulting **measured** bounding box into label placement and content-dependent shape sizing. Extents are never estimated. Rendering a mixed value such as `"filter offset $O_2$"` therefore produces one label, not independently placeable fragments. Role-specific font families, sizes, baseline metrics, and their scene-unit conversion are preset data and must be pinned with the renderer.

### 8.3 Style system

Styling is a two-layer system. Preset tokens provide concise semantic defaults, while exact values let technical diagrams reproduce an established visual language. Both forms are deterministic.

**Layer 1 — preset.** `style <ident>` selects the rendering preset for the whole figure. One built-in (`textbook`): thin ink strokes, small filled point disks, TeX sized relative to figure scale, and a fixed accent palette.

**Layer 2 — classes.** `class id { props }` declares a reusable style class; `[id]` applies it to any op. A class may mix preset tokens and exact values:

| Prop | Values | Default | Applies to |
|---|---|---|---|
| `stroke` | `solid \| dashed \| dotted \| dash(n, n, ...)` | `solid` | stroked geometry |
| `weight` | `thin \| normal \| bold \| number` | `normal` | stroked geometry |
| `accent` | named color or HEX color | `ink` | stroke, point mark, and text |
| `fill` | `none \| tint` or a named/HEX color | `none` | closed geometry |
| `opacity` | number in `[0, 1]` | `1` | the complete rendered op |
| `cap` | `butt \| round \| square` | `butt` | open stroke ends |
| `join` | `miter \| round \| bevel` | `miter` | stroke corners |

A named color is one of `ink | gray | blue | orange | teal | green | red | purple` and is resolved by the preset. A HEX color is an unquoted sRGB literal in `#RRGGBB` or `#RRGGBBAA` form, such as `#2563EB` or `#2563EB80`. The optional final byte specifies alpha as `AA / 255`: `00` is fully transparent and `FF` is fully opaque; omitted alpha defaults to `FF`. Color alpha applies to the corresponding stroke, point mark, text, or fill and composes multiplicatively with the op's `opacity`. Three- and four-digit shorthand forms are not supported. `locus fmt` canonicalizes hex digits to uppercase while preserving the six- or eight-digit form. `accent` colors strokes, point marks, and label/text glyphs. `fill: tint` remains the preset's light wash of the resolved accent, while an explicit fill color is used unchanged.

A numeric `weight` is a finite value greater than zero, measured in CSS pixels; it bypasses the preset's `thin | normal | bold` mapping. SVG emits that value directly, PNG inherits it from the SVG, and TikZ converts it using `1px = 0.75pt`. `dash(...)` accepts at least two finite positive CSS-pixel lengths. An odd-length sequence is repeated once, matching SVG dash-array semantics. `solid`, `dashed`, and `dotted` remain preset-defined patterns.

Values are validated when the class is declared. Non-finite numbers, out-of-range opacity, non-positive weights or dash lengths, an empty/one-element dash sequence, and malformed HEX colors are source-anchored errors. Props that do not apply to an op kind are ignored; for example, `fill` has no effect on a line and `cap` has no effect on a label. Exact values do not weaken byte determinism: identical literals resolve to identical backend values without palette lookup.

For example:

```locus
class signal { accent: #2563EB, weight: 1.5, stroke: dash(6, 3), cap: round }
class region { accent: #0F766E, fill: #CCFBF180, opacity: 0.85, join: round }
```

The four built-ins are ordinary classes with reserved names:

| Built-in | Equivalent declaration |
|---|---|
| `main` | `{ stroke: solid, weight: normal, accent: ink, fill: none }` |
| `auxiliary` | `{ stroke: dashed, weight: thin, accent: ink }` |
| `construction` | `{ stroke: dotted, weight: thin, accent: gray }` |
| `highlight` | `{ stroke: solid, weight: normal, accent: blue, fill: tint }` |

When a class omits a prop, the built-in `main` value supplies the default; an exact class value takes precedence over its preset counterpart. Class names share the file's identifier namespace (unique, declared before use). Semantic built-ins remain the recommended default for textbook geometry; declared classes support dashed lifelines, exact brand colors, and color-coded signal families.

### 8.4 Viewport

- Auto-fit: bounding box of all *visible* geometry + resolved label boxes, plus margin; y-flip to screen space. Points with `mark: none` that are referenced only as anchors do **not** extend the viewport.

### 8.5 Ellipse and B-spline rendering

An ellipse is closed stroked geometry and accepts fill. Its rotated bounding-box half
extents are `sqrt((a cos(theta))² + (b sin(theta))²)` horizontally and
`sqrt((a sin(theta))² + (b cos(theta))²)` vertically. SVG uses a native ellipse with its
rotation transform; PNG follows SVG, and TikZ uses the equivalent rotated ellipse.

A B-spline is open stroked geometry; `fill` has no effect. Convert each nonzero knot span
`[u, v]` to an exact cubic Bézier representation with controls `C(u)`,
`C(u) + (v−u)C'(u)/3`, `C(v) − (v−u)C'(v)/3`, and `C(v)`, in increasing span order.
Here “exact” means the same polynomial curve, evaluated with the pinned `f64` arithmetic,
not an interpolating approximation. SVG and TikZ emit these cubic segments; PNG follows
SVG. The control-point bounding box is a conservative viewport bound, not extra visible
geometry. Control points are only marked when separately declared with a visible mark.

Incidence and measurements always use the analytic definitions, not rendered polylines.
The fixed derivative evaluation, conversion operation order, and curve-aware label
collision procedure must be frozen with the curve implementation (Part 12).

### 8.6 Joint diagram layout and routed paths

Diagram layout resolves measured content, automatic dimensions, relative placement, and routes into one consistent scene. It must not size boxes with guessed text extents, route connectors before finalizing their source/target geometry, or apply clearance only as a final cosmetic adjustment. The numerical method and bounded retry policy remain open in Part 12; the behavioral requirements are:

1. Expand functions and groups in source order and validate constructor overloads, properties, symbolic directions, class arguments, and preset measurements. Resolve typography and measure text before using it in layout constraints.
2. Satisfy authored geometric equalities together with layout requirements. Alignment and insets use measured bounds; ordered gaps require full-bound separation; containment uses actual interiors and style padding. These named layout requirements do not make ordinary numeric inequalities legal in `rel` (§3.3). Fixed dimensions and explicit construction branches remain binding.
3. Choose compact automatic dimensions, appropriate preset proportions, positive default sizes, automatic gaps, and deterministic placement only after satisfying defining facts. Engine preferences may never relax a required relation, enlarge an explicitly fixed dimension, or steer the result using asserts.
4. Resolve automatic routes between the existing endpoints. Curved paths use smooth centerlines; orthogonal paths use axis-aligned spans. For a routed-path `avoid` subject, the routing pass holds obstacle geometry fixed and adjusts the subject's path. It keeps attached ports, excludes obstacle interiors/outlines, and avoids listed connector crossings and overlapping strokes, with automatic style clearance. Explicit endpoint contact remains permitted where an attachment declares it.
5. Place path labels with their measured boxes and `near`/`side` preferences. Text avoidance and route/collection bounds participate in the shared layout: if unconstrained guide spacing must widen to fit a caption, layout may widen it while retaining endpoint incidence and all authored constraints. A route's bounds include its attached captions, but not the external objects referenced by the route. Check all measurement-dependent equalities, including `.length`, against the accepted layout.
6. Apply endpoint clipping to the resolved route against the supported closed boundary's actual outline, including rounded corners. Preserve logical points, incidences, and untrimmed measurements; retain middle crossings and move arrowheads to visible target ends. Keep path labels on the retained visible route. Diagnose fully enclosed paths, incompatible clipping boundaries, and failed intersection or placement requirements.
7. Validate the complete accepted scene before assertions and rendering. Report a layout/routing conflict with source relations and affected member ids when the bounded search cannot satisfy requirements; failure to find a route is not by itself proof that every possible route is infeasible. SVG, PNG, and TikZ use the same resolved routes, captions, corner geometry, and clipped endpoints.

Original route measurements and collection layout bounds use the logical, untrimmed route; clipping changes rendering, not mathematical facts. Resolved visible strokes and labels determine the final viewport. This separation prevents clipping from silently changing a `sample.length` equality or endpoint incidence.

Reports distinguish geometric equality unknowns/rank from automatic dimensions, standalone-label placements, routing choices, and active layout requirements. Preset choices are reported as engine decisions. Groups, functions, aliases, and clipping add no independent placements. Equality-only rank and DOFs remain separate from one-sided active requirements and preference completion (§7.7).

---

### 8.7 Pinned measured lowering, routes, and feedback (normative)


Measured bounds `L, R, B, T`, center `c`, y-up, `pad`/`gap`/`clearance` from the preset.

| Relation | Rows |
|---|---|
| `center(A, B)` | `cx(A) − cx(B)`, `cy(A) − cy(B)` |
| `align_center_x/y`, `align_left/right/top/bottom` | component difference from the first argument, n−1 rows |
| `inset_left(A, B)` (`{ padding: none }` → `pad = 0`) | `L(A) − (L(B) + pad)`; mirrored for the other sides |
| `gap_x(A, B, …)` | `L(Bᵢ₊₁) − R(Bᵢ) − gap ≥ 0` per adjacent pair; preference `= gap` |
| `gap_y(A, B, …)` | `B(Aᵢ) − T(Aᵢ₊₁) − gap ≥ 0` |
| `row` / `column` | collection expansion, then `gap_*` chain + `align_center_*` |
| `cover_x(C, items)` | `minL(items) − pad − L(C) ≥ 0`, `R(C) − maxR(items) − pad ≥ 0`; `cover_y` likewise |
| `contain(C, items)` | each item's four box corners: `−sdf_C(P) − pad ≥ 0` (below) |
| `on(P, edge(rect, W))` | existing segment residual on a derived `DLinear` |
| numeric `==` on `.width`, `.height`, `.length`, `style_metric` | existing numeric equality over `Jet` values |
| `avoid(subject, obstacles…)` | member-wise rows `sep(A, B) − clearance ≥ 0` per pair; see [Avoidance](#avoidance) |
| `trim_endpoints` | no rows |

Operand bounds: point → zero size; shapes → from center and dimensions; segment → endpoint
box; arrow → `Route` bounds ∪ attached label boxes; standalone label → center ± measured
half size; group / instance → live-selected union of member bounds.

**Hard-face representation for settling.** Recursively enumerate collection/instance
members in declaration order and deduplicate canonical alias identities. Keep a route's
geometry and each attached caption separately identifiable, including when a caption is
also an explicit collection member. Straight/polyline route geometry contributes its
endpoints and bends. A quadratic contributes both endpoints and its x/y stationary
points with parameters clamped to `[0,1]` (use parameter zero for a zero denominator).
These candidates have fixed identities even when the extreme changes. Finite segments,
unboxed polygons and B-spline control boxes contribute individual points; automatic
shapes and measured label boxes retain their own bounds. Ordinary aggregate bounds
remain the measurement, equality and preference representation.

For each adjacent horizontal gap, emit `L(b) − R(a) − gap ≥ 0` for every contributor pair
`(a,b)`; vertically emit `B(a) − T(b) − gap ≥ 0`. Their conjunction is exactly the
aggregate inequality. Emit **one** original aggregate compact-gap preference per
adjacent operand pair, and preserve the aggregate row/column center equality. For cover,
emit both padded axis faces per content contributor against the container's aggregate
bounds. For axis-aligned rectangular containment, emit all four padded faces per content
contributor rather than a selected nearest signed-distance face. Row order is source
order, adjacent-pair order, contributor order, then low/high x and low/high y faces.
Identity deduplication is structural, never based on current equal coordinates or
derivatives, so row identities and source/member provenance remain stable between trials.
Gap expansion is quadratic in the two contributor counts; cover and rectangular
containment are linear. Phase 1 retains the original aggregate residual weights.

For diamonds, ellipses, rounded rectangles and other nonrectangular containers, retain
the four **combined-box** corner signed-distance rows per original content operand.
Member-wise containment is not equivalent: the combined box can have synthetic corners
outside the union of its members. These rows keep live geometric piece selection.

#### Signed boundary distance

"Shrunk along inward normals by `pad`" is exactly the set of interior points whose Euclidean
distance to the boundary is at least `pad`. `sdf_C(P)` is the **exact Euclidean signed
distance** from `P` to `C`'s outline, negative inside, pinned per shape:

- rect: `max_live(|dx| − w/2, |dy| − h/2)` inside; corner/edge distance outside.
- rounded rect: the existing exact signed distance (`geom/rounded_rect.rs`,
  `solve/ad/rounded_rect.rs`).
- diamond: exact distance to the rhombus outline (vertex/edge pieces, live-selected).
- circle: `|P − O| − r`.
- ellipse: exact nearest-point distance by the pinned algorithm (Eberly's robust root-finding
  on the ellipse parameter, fixed iteration count, deterministic), negated inside.

**Ellipse derivatives.** Let `Q` be the nearest boundary point at parameter `φ*`, with
`Q = O + a cos φ* U + b sin φ* V`, and let `n` be the outward unit normal at `Q`
(`(u_Q/a², v_Q/b²)` in local coordinates, normalized, mapped through `U, V`). By the
envelope theorem:

- `∂sdf/∂P = n` when `|P − Q| > 0` this equals `±(P − Q)/|P − Q|`; **at the boundary
  (`P = Q`) the quotient is `0/0` and `n` is used directly**. Signed distance is `C¹` across a
  smooth boundary, so approaches from inside and outside agree.
- `∂sdf/∂O = −n`, `∂sdf/∂a = −n · (cos φ* U)`, `∂sdf/∂b = −n · (sin φ* V)`,
  `∂sdf/∂θ = −n · (a cos φ* V − b sin φ* U)`. The same expressions hold at zero distance.
- On the medial axis (the major-axis segment between the evolute cusps, which contains the
  center when `a > b`) two nearest points tie; the live piece picks the one with nonnegative
  local `v`.

Pinned boundary cases: center of `(4, 1)` (distance `1`); `(1, 0)` in `(4, 1)` (distance
`≈ 0.9661`); boundary points at both vertices and at a generic parameter, approached from
inside and outside with matching one-sided gradients; near-medial-axis ties; `a = b`.

**Empty inset regions.** The `pad`-inset region of `C` is empty iff `pad > inradius(C)`
(the largest inscribed circle: `min(w, h)/2` for a rect, `r` for a circle, `b` for an ellipse,
`wh/(2√(w² + h²))` for a diamond). At `pad = inradius` the region is the **set of deepest
points** (`sdf = −pad`, row exactly `0`): a single point for a circle, square, diamond, or
ellipse with `a > b`, but the segment `[−(w − h)/2, (w − h)/2] × {0}` for a `w > h` rect (a
centered `10 × 2` rect with `pad = 1` keeps `[−4, 4] × {0}`, so a horizontal segment of
length up to 8 still fits). Whether content fits is decided only by its corner rows, never by
a separate emptiness test. An empty region is a layout conflict
for fixed-dimension containers and an infeasible row for automatic ones. The ellipse's
implicit `on` residual (§3.3) is not used for padding.

#### Avoidance

`avoid` dispatches member-wise over subject and obstacle collections (instances contribute
owned visible members; groups contribute normalized members). Each pair contributes one row
`sep(A, B) − clearance ≥ 0`, where `sep` is a **shape-aware signed separation** computed in
two steps. Bounding boxes are a fast rejection test only, never the constraint.

1. **Reduce.** Strokes carry half their width and circles/rounded rects are reduced to their
   core shape by Minkowski decomposition (`sep(A ⊕ disk(ρ), B) = sep(A, B) − ρ`). Every core is
   a point, a segment, or a convex polygon with nonempty interior.
2. **Classify and measure by core kind.** Degenerate cores are handled explicitly because they
   contribute no, or too few, edge normals:
   - *point–point*: `sep = |P − Q|` (never overlapping).
   - *point–segment*: `sep` = distance to the closed segment (live-selected endpoint/interior
     piece; never overlapping).
   - *segment–segment*: proper or touching intersection by orientation tests (including the
     collinear-overlap case) ⇒ overlapping with `sep = −` the smaller escape distance along the
     two segment normals; otherwise disjoint with `sep` = minimum of the four endpoint–segment
     distances. Two collinear segments separated along their length are correctly disjoint
     here, whereas their shared normal alone cannot separate them.
   - *point–polygon*: inside iff on the inner side of every edge half-plane; inside ⇒
     `sep = −min` inward edge distance (escape distance), outside ⇒ distance to the outline.
   - *segment–polygon* and *polygon–polygon*: separating-axis test with axis set = edge normals
     of both shapes **plus the direction axis of any segment core**. For each unit axis `u`
     compute the **directional escape distance** `e(u) = min(max_u A − min_u B, max_u B − min_u A)`,
     i.e. how far one shape must move along `±u` to separate. If some axis has `e(u) < 0` the
     pair is disjoint and `sep` = minimum vertex–edge feature distance (live-selected; exact
     for convex pairs). Otherwise the pair overlaps and `sep = −min_live e(u)`; the minimizing
     axis is the penetration direction. `e(u)` is deliberately not the interval-intersection
     length: a segment crossing a box has zero-width projection but nonzero escape distance.

Examples: segment `(−2, 0)–(2, 0)` through the box `[−1, 1]²` has every vertex–edge distance
`≥ 1`, but no axis separates it and the `y` escape distance is `min(0 − (−1), 1 − 0) = 1`, so
`sep = −1`. Two unit circles offset by `(1.5, 1.5)` reduce to point–point with
`sep = √4.5 − 2 ≈ 0.1213` and satisfy a `0.1` clearance even though their boxes overlap on
both axes.

| Pair | Treatment |
|---|---|
| label box, rect, diamond | convex polygons with interior |
| segment, polyline span, arrow route span | segment cores |
| hidden point, circle center | point cores |
| circle | point ⊕ disk(`r`) |
| rounded rect | inner rect ⊕ disk(`r`) |
| general simple polygon | per-edge strokes for crossings plus point-in-polygon containment of the subject's corners; penetration for a fully contained subject is `−min` corner distance to the outline (documented approximation for non-convex obstacles) |
| ellipse vs point-like (label corner, circle center) | exact nearest-point distance |
| ellipse vs segment/polygon | conservative: the ellipse's exact rotated bounding box stands in for the outline; documented limitation of this version |

- **routed path as subject**: no rows; handled by the discrete route search, with the
  measurement loop closing the dependency.
- **penetration direction**: for an overlapping pair the minimizing escape axis is
  live-selected with lowest-index tie-breaking. Because `sep` is non-convex, a local solve can settle on the
  wrong side of an obstacle; that case uses the [outer discrete search](#outer-discrete-search),
  which forces the next penetration direction for the lowest-index conflicting pair. Disjoint
  pairs need no side choice.

#### Shared route representation

One `Route` type in `locus-core` (`geom/route.rs`) is the single source of bounds, length,
sampling, and rendering geometry for every arrow, evaluated as `Jet`s during the solve and as
`f64` afterwards:

- `Direct { bend }` — endpoint segment or quadratic Bézier. Bounds come from the curve's exact
  axis extremes (parameter `t = (P0 − P1)/(P0 − 2P1 + P2)` per axis, clamped to `[0, 1]`,
  live-selected), not the endpoint box. Length: segment distance or closed-form quadratic arc
  length.
- `Orthogonal { side, topology, d }` — axis-aligned span sequence derived from the endpoints,
  side, topology, and detour unknown `d`; for the three-span right detour
  `tail → (x_d, y_tail) → (x_d, y_tip) → tip` with `x_d = max_live(x_tail, x_tip) + d`. Length
  is the sum of actual span lengths (`|x_d − x_tail| + |y_tip − y_tail| + |x_d − x_tip|`), valid
  for differing endpoint x.
- `Curved { side, k }` — quadratic Bézier whose control is the chord midpoint offset by `k`
  perpendicular toward `side`; exact extremes and closed-form length as for `Direct`.
- Attached labels: measured box at route parameter `t` offset by `o` on the page side; `t` and
  `o` are the label-parameter unknowns above. The box is part of the path's bounds.

Continuous route/label parameters (`d`, `k`, `t`, `o`) are unknowns of the continuous solve;
discrete parameters (side, topology, retained-route candidate) are frozen within a solve pass.
Route **coordinates** are always functions of the current endpoints and parameters, so a route
never detaches from endpoints that move during a re-solve.

#### Outer discrete search

Discrete route and avoidance choices are enumerated by a bounded, deterministic schedule that
runs in two situations — after a continuous solve fails Phase 1, and when the discrete layout
pass reports a conflict:

1. Identify the lowest-index materially failing row whose implicated unknowns or measurements involve a
   route, label, or avoidance pair (`relation_reports`/`implicated` already provide this for
   geometric rows).
2. Advance that element's discrete choice to the next candidate in preset order: route side
   `auto → right, left, above, below` (filtered by the authored `side`), topology
   `3-span → 5-span` for orthogonal routes, penetration direction for an overlapping
   avoidance pair, retained-route candidate for a clipped label.
3. Re-run the continuous solve. Candidates are visited in lexicographic order over elements
   and choices with a total cap of 32; exhausting them reports the original failing rows as
   a layout/routing conflict (exit 2).

This schedule does not depend on an `avoid` relation being present; a plain
`route: orthogonal` whose authored `.length` cannot be met with the default side or topology
is covered.

#### Measurement feedback loop

After each successful continuous solve, the discrete layout pass runs: route search for
`avoid` with a routed-path subject, path-label candidate selection within the solved `t`/`o`
region, `trim_endpoints` clipping, and avoidance validation. It may change discrete route
choices, label candidates, and the retained visible route on which a label sits. Then:

- Recompute every solve-time measurement (route bounds, label boxes, collection bounds,
  `.length`) from the accepted choices.
- If any differs from the values the solve used beyond `1e-12 × S`, re-run the continuous
  solve with the new discrete choices frozen. Repeat up to a preset pass cap (3).
- On convergence, the solve's route equals the accepted route: `rel path.length`, asserts,
  collection bounds, and rendering all use the **accepted logical route** and the **accepted
  label placement**. Preset defaults exist only as first-pass seeds; there is no separate
  nominal semantics.
- Clipping never changes `.length` or logical bounds (§8.6), but it can move a retained-route
  label; that change participates in the loop like any other.
- Exhausting the pass cap is a layout/routing conflict (exit 2) naming the oscillating
  measurements and their relations.


Search priority considers rows above `max(1e-12, worst_residual * 1e-8)` so small numerical noise does not hide a later material failure. This changes only candidate priority: accepted feasibility remains strictly `max|r| < 1e-12`. Joint retries retain other accepted/forced choices, honor authored side and topology, and preserve the original failure diagnostic when exhausted. Route offset enlargement factors are `1,2,4,8,16,32,64,128`. Actual route probes share a global 32 budget across bootstrap and measurement passes; unsuccessful/recovered continuous probes are counted.

Feedback compares proposals with the actual accepted continuous choices, including outer-search retries. If a core retry replaced the requested label candidate, that just-rejected input candidate is excluded from the immediate visual proposals for the same solve; the accepted placement remains the baseline, its warnings remain reportable, and every distinct proposal still requires an ordinary solve. New route floors, label candidates or physical prefix/suffix arc-length trims always require an ordinary solve. Native retained parameters describe rendering; physical trim distances have shared primal, AD and assertion semantics. Clipping leaves logical route length and logical bounds, including attached labels, unchanged. Only one initial clipping-related E413 may obtain positive trim choices from an explicitly unsolved relation-aware seed; ordinary solving and final layout must then succeed, or the original diagnostic is retained. Measurement changes use `1e-12` times the characteristic scale, with three passes maximum.

Label candidates score visible strokes and text overlap, respecting intentional clear container interiors. If equality-only DOF analysis determines both attached-label parameters, retain their accepted candidate: a nominal shift cannot override those hard source facts. Collision warnings are still rebuilt from the actual visible scene. Explicit solid avoidance remains a hard constraint. Prediction uses proposed routes and visible obstacles. Curved collision spans use the same conservative quadratic/cubic subdivision in primal and AD. Endpoint contact is allowed only if its outward prefix stays outside; inward excursions remain collisions.

#### Pinned textbook presets

`locus_core::preset` is the shared implementation of these values. All lengths below are scene units, except typography sizes in ems and normalized tolerances.

| Value | Pinned constant |
|---|---|
| Automatic box width/height | `2 / 1` |
| Padding and gap | `0.25 / 0.25` |
| Avoidance clearance | `0.15` |
| Scene units per em; normal/title size | `0.28`; `1 / 1.4` |
| `short_length`, `narrow_width` | `1 / 0.3` |
| Collision stroke width; margin; mark size | `0.04 / 2 / 0.2` |
| Automatic rounded radius | `0.15 * min(width,height)` |
| Route detour/control default; positive minimum | `0.5 / 0.5`; `1e-6` |
| Label default `t`; source/middle/target regions | `0.5`; `[0.1,0.3] / [0.4,0.6] / [0.7,0.9]` |
| Route page side order; signed side order | right, left, above, below; `+1,-1` |
| Label page side order | above, below, right, left |
| Active/stop/direction tolerance | `1e-7 / 1e-9 / 1e-14` |
| Armijo; minimum alpha; accepted-move/restoration caps | `1e-4`; `1/1024`; `256 / 8` |
| Consecutive working-set pivots; total settling iterations | `64 / 1024` |
| Curvature rebuild attempts; gradient trigger; difference step; tangent-dimension cap | `1`; `1e-5 × max(1, ‖q‖∞)`; `1e-4`; `32` |
| Rebuild probe correction / step; rebuilt / ordinary direction infinity-norm bound | `0.1 / 8` |
| Outer search; avoidance directions; measurement passes | `32 / 16 / 3` |
| Search row priority factor | `1e-8` |
| Ellipse nearest-point bisections | `96` |
| Clipping depth, bisections, interval cap | `48 / 64 / 16384` |
| Attachment bracket subdivisions/prefix interval cap | `256 / 2048` |
| Conservative curve collision subdivisions | `64` |

Text measurement uses bundled Noto Sans Regular with Noto Sans CJK Simplified Chinese Regular fallback for normal/title plain text, and the bundled Rex math fonts for TeX, with exact measured extents passed into core; core has no font-engine dependency. The checked corpus fixture pins those extents as binary64 bits and validates them against the bundled fonts. SVG, TikZ and PNG consume the same accepted expanded styles and geometry.

## Part 9 — CLI

### 9.1 Commands

| Command | Purpose |
|---|---|
| `locus init <file> [--style textbook]` | create a new scene file |
| `locus add <file>` | append statements from stdin; whole file re-validated |
| `locus update <file> --id X --def "<rhs or (args)>"` / `--set 'dir=NE'` | surgical edit of a declaration's `= rhs` or constructor arguments; re-solve |
| `locus remove <file> --id X` | remove; orphaned dependents → exit 2 |
| `locus describe <file>` | structural dump: objects, defs, anchor ownership, relative geometry, solve summary, DOF report, numeric-hint changes, branch resolutions |
| `locus inspect <file> [--distance A B] [--angle A B C] [--ratio A M B] [--measurements] [--incidences]` | diagnostics and explicitly requested measurements/incidences |
| `locus check <file>... [--strict]` | solve + diagnostics + all in-file asserts; warnings are non-fatal unless strict — the CI command |
| `locus fmt <file> [--json]` | canonical formatting / AST-as-JSON export |
| `locus render <file> -o out.svg [--png] [--tikz] [--stdout]` | render (intended for humans, not agent feedback) |

`check` measures text with bundled fonts whenever measured content exists and validates accepted layout before assertions. `update --id` accepts top-level object, instance and group ids: instance `--def "function(arguments)"` preserves the name, class and trailing comment; group `--def "group(members...)"` preserves its name. Properties are editable only on object declarations. Qualified member edits return E305 naming the body line and invocation chain. Function definitions are edited directly in source. Removal uses expanded dependencies, including groups, call arguments and external member references; dependent declarations/relations/asserts return E303. A mutation is written only after the full candidate validates.

Since the format is line-oriented, agents may equally edit the file directly; `check` validates everything regardless of how edits happened. CLI ops are the "safe API," direct editing the "expert mode."

### 9.2 Exit codes

| Code | Meaning |
|---|---|
| `0` | success / all asserts pass |
| `1` | one or more asserts failed, or `check --strict` found warnings |
| `2` | solve, layout, routing, or clipping failure; construction/dependency error |
| `3` | usage / parse error |

### 9.3 Output conventions

- All commands support `--json` for machine-readable output (agents); default output is human-readable with source-line anchors.
- All commands support `--json --compact` for single-line JSON. Compact `check` retains every diagnostic and failed assertion (including all actual subterms), and summarizes solver unknowns, equations, rank, remaining global DOF, and maximum normalized residual. Its assertion report includes `passed`, `total`, `failed`, and `assertions` containing only failed results. Omit `--compact` for full solver and assertion details. Exit codes and strict-warning behavior are identical in both modes.
- `describe` and `inspect`, including their JSON forms, expose object relationships, dimensions, anchor identities, relative measurements, and branch choices. These provide the information needed to author or repair a figure. Internal coordinates belong to generated rendering output (§7.6).
- `inspect --json` reports diagnostics (`degenerate`, `hint_far`, `label_collision`, `out_of_view`, `crowding`) by default, with empty measurement tables and incidences. Compact inspection omits empty tables and incidences. Repeatable `--distance A B`, `--angle A B C` (vertex B; radians), and `--ratio A M B` (`AM/MB`; M strictly inside AB) request only those tuples. Unknown/non-point ids or undefined measurements return exit 3 with an `inspect_query` diagnostic. `--measurements` opts into exhaustive tables (pairwise distances among named points, triangle angles, ratios along shared lines) and conflicts with direct tuple queries. `--incidences` independently opts into all detected incidences ("`inc` tangent to `seg(A,B)`", "`M` on `AB` with `AM:MB = 1:1`"). `check` computes diagnostics without enumerating measurement tables or incidences.
- Visual `crowding` requires two visible point markers; `mark: none` points do not produce `crowding` or `out_of_view` diagnostics. Hidden helpers remain part of mathematical measurements, incidences, and assertions. Geometry and solve diagnostics do not depend on marker visibility.
- Declarations do not require incoming source references. Terminal drawing objects are intended outputs; unused numeric declarations and hidden construction objects are also valid. An absent reference alone produces no diagnostic and cannot fail `check --strict`.
- On assert failure, the report includes the actual value of **every measurable subterm** so the agent can localize which upstream definition is wrong.
- Diagram reports include automatic versus explicit dimensions, measured label sizes and roles, attachment ids, `near`/`side` choices, route modes and logical lengths, clipping boundaries, collection membership, and failed layout/clearance requirements. They identify qualified function members and source chains. `check` validates routing, containment, and clipping requirements before `render`; these failures return exit 2 with source-anchored layout/routing diagnostics. Ordinary annotation-collision warnings retain the existing strict-check policy. Public reports describe accepted choices through object identities and measurements.
- Ellipse reports include semiaxes, rotation, anchor identities, and area. B-spline reports include the ordered control expressions, degree `3`, derived knot vector, span count, and endpoint identities. Detected incidence for ellipses uses §3.2; B-spline incidence is not inferred from samples or its control polygon.

### 9.4 JSON interchange

`locus fmt --json` emits the AST: version, segmented meta text, style preset, declared classes, ordered ops (`{id, kind, def, where, attr, props}` with `def` as both source string and parsed tree; a constructor declaration's `def` is the constructor call), rels, and asserts. A free numeric op additionally carries its scalar `hint`. Placement ownership follows from the constructor overload. Numeric expression nodes include their inferred dimension (§6.1); text nodes contain the ordered plain/TeX segments from §1.4. This is the integration surface for future MCP shims, editors, or watch-mode preview servers that wrap the CLI.

For reusable functions (§1.5), preserve ordered definitions, parameter names, body statements, and named invocations with their arguments and optional class. Also expose expanded ops, rels, and asserts with qualified ids and origin information linking each item to its body statement and invocation chain. Definition templates are not scene objects and contribute no unknowns. `fmt` preserves the reusable source form; `describe` and diagnostics identify expanded members such as `approved.payment.path`. `functions`, `calls`, and `groups` retain ordered source AST records. `collections` is keyed by qualified id and contains `id`, `kind` (`group` or `instance`), normalized ordered `members`, `span`, and `origin`. `expansion_origins` aligns with expanded statement order and contains the body `span` plus ordered `invocations` (`function`, qualified `instance`, call `span`). `expanded` contains the flat interchange document; `relation_properties` associates props with `relation_index`. Empty extension fields are omitted.

For groups (§1.6), preserve ordered declarations and authored member references, and expose the normalized member identities after nested-group and instance expansion. Group metadata describes membership separately from drawable ops. `fmt` preserves group declarations; `describe` reports membership and the relations involving the collection, and removal checks include group references as dependencies. Local group records retain qualified ids and expansion origins. `describe` attaches nonempty invocation `origin` records to expanded objects and includes `collections`, `expansion_origins`, expanded `relations` and `relation_properties`, and `diagram` records with qualified `id`, canonical expanded `definition`, authored `properties` and coordinate-free `accepted` details. Box details distinguish automatic dimensions and width/height; labels report measured width/height, typography role and accepted path side/region; arrows report accepted route mode, side, logical length and clipping. Collection relations use the expanded relation trees.

Preserve zero-argument shape/label constructor forms, label attachment mode and typography role, route/placement properties, symbolic `edge` directions, and `style_metric` calls in the source AST. Preserve automatic dimensions and engine-owned placement in their source forms. Resolved reports may include measured dimensions and logical path lengths; coordinates and control arrays are restricted to generated rendering output. Solve reports include equality DOF classes, active requirements, settling completion/reason/projected gradient and separate settling, restoration, outer-search, routing and measurement counters. E413 conflicts name source requirements and implicated members; incomplete settling is a warning and fails strict checks.

### 9.5 Intended agent loop

```bash
locus init fig.locus
locus add fig.locus < ops.locus
locus check fig.locus --json    # → solve summary + DOF report + assert results
# wrong branch? → change the explicit construction or tangency predicate.
# unresolved relative DOF? → add a relation. conflict? → revise the defining facts.
locus render fig.locus -o fig.svg
```

Recommended agent policy: **never `render` until `check` exits 0.**

---

## Part 10 — Out of scope for 0 beta 6 (reserved)

- Inequality relations in `rel` (only asserts may compare with `< > <= >=`).
- General ellipse/B-spline intersections, curve tangency and projection; ellipse arcs and perimeter; variable-degree, custom-knot, rational, periodic, closed, or fitted B-splines (§5.10–§5.11).
- User-defined presets or named palettes; per-op inline style declarations; whole-figure transforms; layers; animation; 3D. Exact values remain reusable through classes (§8.3).
- Statement continuations or multi-line simple statements; includes/imports; recursive or expression-valued user-defined functions. Reusable statement blocks and named calls are specified in §1.5.

The file header identifies the language draft and will change when a future draft makes incompatible changes.

---

## Part 11 — Complete examples

### 11.1 Geometric construction

```locus
locus 0 beta 6

meta title = "Externally tangent circles with a common external tangent"

style textbook

// ---- objects ----------------------------------------------
num r_1 = 2
num r_2 = 3

circle c1(r_1)
circle c2(r_2) where { align_center_y(c1.center, c2.center) }
point O_1 = c1.center
point O_2 = c2.center

rel tangent(c1, c2)                     // explicitly external tangency

// contact point divides the center segment in the ratio of the radii
point T = lerp(O_1, O_2, r_1 / (r_1 + r_2))

// external homothety center — direct, so constructed
point E = lerp(O_1, O_2, r_1 / (r_1 - r_2))

// common external tangent: through E, with an explicit construction branch
line t_ext = tangent_line(E, c1, 0)   [highlight]
point P_1 = foot(O_1, t_ext)
point P_2 = foot(O_2, t_ext)

// ---- annotation -------------------------------------------
label l1(O_1, $O_1$)  { dir: S }
label l2(O_2, $O_2$)  { dir: S }
label lT(T, $T$)      { dir: SE }

// ---- intent -----------------------------------------------
assert dist(O_1, O_2) == r_1 + r_2
assert on(T, c1)
assert on(T, c2)
assert between(O_1, T, O_2)
assert tangent(c2, t_ext)               // touches BOTH circles — the theorem
assert collinear(E, P_1, P_2)
```

The agent states radii and a tangency relation, then derives anchors, the contact point, and a common tangent. The assertions verify their consequences.

### 11.2 Relational diagram

```locus
locus 0 beta 6

style textbook

fn labeled_rect(text) {
    rect shape()
    label caption(text)
    rel center(caption, shape)
    rel contain(shape, caption)
}

label title("Validate before rendering") { role: title }
labeled_rect check("locus check")
labeled_rect render("locus render") [highlight]
group nodes(check.shape, render.shape)
rel column(nodes)
rel align_center_x(title, nodes)
rel gap_y(title, nodes)

arrow success(check.shape.s, render.shape.n)
label yes(success, "pass") { near: start, side: left }
arrow retry(render.shape.e, check.shape.e) { route: curved, side: right }
label again(retry, "retry") { near: middle, side: right }
rel avoid(retry, success)
```

Shape sizes, gaps, standalone label placement, and return-route controls are engine-selected. Function members remain addressable, group membership does not draw extra objects, and connector captions use the accepted visible paths.

The two composition examples in §0.2 are broader acceptance cases: together they exercise orthogonal loops, class arguments and nested calls, preset path/overlay dimensions, finite rectangle edges, content-dependent frames, grouped avoidance, and endpoint clipping. Both are executable current-format programs.

---

## Part 12 — Open questions before freezing

1. **Placement initializer — pinned** in §7.7, including ring seeds, relation-aware passes and bounded precondition retry scales.
2. **Rigid symmetry classification** — common translation is handled by §7.2. Define when common rotation is also presentation-only, accounting for axis-alignment relations, compass anchors, relative offsets, and angle-based constructions, before exempting it from strict checks.
3. **Solver constants** — empirically validate the specified normalization, rank threshold, damping schedule, and `1e-12`/`1e-9` gap on extreme-scale figures before freezing `0 beta 6`; any change after freezing requires a new file header.
4. **Implementation migration — complete** for parser/AST, constructor typing, unknown ownership, solver initialization, CLI/JSON reports, and fixtures. Relative geometry is expressed through relations and explicit construction branches.
5. **TeX subset boundary** — which TeX commands are guaranteed? Needs an explicit whitelist appendix before v1.
6. **Curve implementation** — add ellipse/B-spline syntax and typed overloads, parameter domains, analytic evaluation and differentiation (including B-spline endpoint derivatives), rotated/conservative bounds, exact cubic span conversion, and curve-aware layout. Pin conversion operation order and collision tolerances; verify endpoint, interior-knot, repeated-control, and rotated-ellipse cases across renderers. Ellipse and B-spline demonstrations now pass runtime acceptance and renderer parity; the remaining freeze review concerns conversion operation order and extreme-scale coverage.
7. **Measured layout and path operations — pinned** in §7.7–§8.7: live hinges, hard feasibility, working-set preferences, exact boundary distances, bounded search, measurement feedback and honest incomplete-settling reports.
8. **Function expansion, source JSON and CLI editing — pinned** in §1.5 and §9.1–§9.4. Independent, nested, object/class arguments, source tracing and qualified-member edits are validated.
9. **Ordered groups — pinned** in §1.6, §8.7 and §9.4: normalized membership, atomic instance row/column operands, aggregate bounds, recursive member avoidance, source chains and removal dependencies.
10. **Diagram presets and rendering constants — pinned** in §8.7, with accepted-route SVG/TikZ/PNG parity and the 28-source corpus. Rounded-rectangle preference settling can stall while remaining feasible; strict checking retains its warning.

### Decided (previously open)

- **Object-owned placement** — `point()` declarations, circles, ellipses, rectangles, rounded rectangles, and diamonds introduce internal placement; anchors and constructive points remain derived (§2.2).
- **Ellipses** — positive ordered semiaxes, optional explicit rotation, derived anchors, incidence, containment, and area (§5.10).
- **B-splines** — derived open clamped uniform cubic curves, with ordered existing control points and scalar parameter sampling (§5.11).
- **Explicit tangency semantics** — `tangent(c1, c2)` is external; `internally_tangent(outer, inner)` is internal, independent of initialization (§3.2).
- Unresolved relative DOFs report as **info** by default; `--strict` promotes. Common translation alone never fails strict checking (§7.2–§7.3).
- **Unified relation vocabulary** — 32 entries follow the relation diagram, including measured alignment, separation, enclosure, clearance, and clipping. `center`, axis-center alignment, and numeric length equality express point coincidence, alignment, and equal lengths (§3.2).
- **Reusable local functions** — `fn` defines a statement block with positional arguments and no return value. Named calls expand fresh local declarations and shared relations; qualified members, nested composition, style fallback, and measured instance collections follow §1.5.
- **Constructor declarations** — `kind id(args)` is the only way to call the constructor named after the kind; `kind id = rhs` is reserved for aliases, differently named derived constructors, expressions, and `free` nums (§1.4).
- **Ordered groups** — `group name(members...)` aliases existing objects and collections, flattens nested membership in order, and keeps the first occurrence of each object identity. Groups draw nothing, introduce no unknowns, and impose no layout; relation-specific expansion or combined bounds follow §1.6.
- **Automatic box sizing** — zero-argument rectangle, diamond, and rounded-rectangle constructors obtain positive compact dimensions from measured content, defining relations, and preset defaults; explicit dimensions stay fixed (§5.5).
- **Measured label modes and typography** — standalone content, point-anchor attachment, and path attachment share measured text; path labels use `near`/`side`, and `role: title` selects preset typography without prescribing placement (§5.6–§5.7).
- **Generic automatic routes** — curved and orthogonal arrows preserve endpoint identities, choose controls internally, and participate in clearance, path-label placement, and endpoint clipping (§8.6).
- **General diagram measurements** — preset `short_length`/`narrow_width`, logical arrow length, and finite `edge(rect, direction)` values provide shared measurements for composition (§5.8, §6.2).
- **Numeric parameters** — scalar hints initialize unknown numeric parameters (§2.2–§2.3).
- **Style classes introduced** — reusable classes with a closed property set (§8.3).
- **Exact class styles supported** — HEX colors, numeric line weights, custom dash arrays, opacity, caps, and joins coexist with preset tokens (§8.3).
- **Mixed text and TeX supported** — quoted strings may contain measured inline `$...$` segments while bare `$...$` remains the all-TeX shorthand (§1.4, §8.2).
