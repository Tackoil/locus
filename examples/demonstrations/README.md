# Demonstrations

This directory contains **42 self-contained figures** using `locus 0 beta 6`: eleven
geometry and technical examples, and one example for each of 31 diagram types.
The agent-loop and checkout figures are the specification's reusable composition
examples. Each figure has one canonical source in this directory.

## Geometry and technical examples

| File | Demonstrates |
| --- | --- |
| [aec3_filter_alignment.locus](aec3_filter_alignment.locus) | Filter-bank alignment and delay annotations |
| [angle_bisector_ratio.locus](angle_bisector_ratio.locus) | Triangle on a placed circumcircle, derived intersection, and the angle-bisector theorem |
| [bspline_controls.locus](bspline_controls.locus) | Five derived controls, two cubic spans, clamped endpoints, and an interior-knot sample |
| [ellipse_foci.locus](ellipse_foci.locus) | Rotated semiaxes, foci, containment, and area scaling |
| [euler_nine_point.locus](euler_nine_point.locus) | Euler line and nine-point circle constructions |
| [inscribed_angle.locus](inscribed_angle.locus) | Inscribed-angle geometry and arc marks |
| [pentagon_golden.locus](pentagon_golden.locus) | Regular pentagon and golden-ratio relationships |
| [point_relations.locus](point_relations.locus) | Parameterless points, relative placement, a 3-4-5 triangle, and a derived circumcircle |
| [rounded_rect.locus](rounded_rect.locus) | Explicit rounded boxes/capsules plus an automatically sized diamond, measured caption, and curved connector |
| [sequence_check.locus](sequence_check.locus) | Explicit message geometry plus generic function instances, frame/tab layout, finite edges, preset dimensions, and clipped paths |
| [tangent_circles.locus](tangent_circles.locus) | Solved tangent circles and common tangents |

## Diagram examples

The table records the Mermaid documentation examples that inspired the figures,
preserving their subjects, data, relationships, and approximate scale. These are
source references; running the Locus examples requires no Mermaid or D2 executable.
The figures use Locus geometry, measured labels, local functions, and layout
relations. Colors, routing, and spacing may differ from the reference examples.

| Type / Mermaid documentation | Mermaid reference example | Locus figure | Scope |
| --- | --- | --- | --- |
| [Flowchart](https://mermaid.js.org/syntax/flowchart.html) | Existing agent-loop variant | [flowchart_agent_loop.locus](flowchart_agent_loop.locus) | Retains the existing check/render workflow, decision branch, and curved repair loop used by the specification. |
| [Sequence](https://mermaid.js.org/syntax/sequenceDiagram.html) | Existing checkout variant | [checkout_sequence.locus](checkout_sequence.locus) | Retains the existing four-participant checkout, alternative branches, activations, and clipping contract. |
| [Swimlanes](https://mermaid.js.org/syntax/swimlanes.html) | Basic example · request/triage/engineering | [swimlanes.locus](swimlanes.locus) | The documented three responsibility lanes, six activities, Known issue branch, and Needs code change handoff. |
| [Class diagram](https://mermaid.js.org/syntax/classDiagram.html) | Animal example | [class_diagram.locus](class_diagram.locus) | The documented Animal superclass, Duck/Fish/Zebra subclasses, attributes, methods, inheritance, and notes. |
| [State diagram](https://mermaid.js.org/syntax/stateDiagram.html) | Default theme, look and layout · Draft/Review/Published | [state_diagram.locus](state_diagram.locus) | The documented Draft, Submitted, nested Review(Screening→Decision), Published, and rejected return to Draft. |
| [Entity relationship](https://mermaid.js.org/syntax/entityRelationshipDiagram.html) | Default theme, look and layout · order model | [entity_relationship.locus](entity_relationship.locus) | The documented four entities, attributes, and places/contains/appears-in cardinalities. |
| [User journey](https://mermaid.js.org/syntax/userJourney.html) | My working day | [user_journey.locus](user_journey.locus) | The documented five tasks, two journey sections, actors, and 1–5 satisfaction scores. |
| [Gantt](https://mermaid.js.org/syntax/gantt.html) | A Gantt Diagram | [gantt_chart.locus](gantt_chart.locus) | The documented four tasks and two sections, with Jan 1/Jan 12 starts, 30/20/12/24 day durations, and the after dependency. |
| [Pie](https://mermaid.js.org/syntax/pie.html) | Pets adopted by volunteers | [pie_chart.locus](pie_chart.locus) | The documented Dogs=386, Cats=85, Rats=15 dataset. Filled circular boundaries use chords of at most five degrees. |
| [Quadrant](https://mermaid.js.org/syntax/quadrantChart.html) | Reach and engagement of campaigns | [quadrant_chart.locus](quadrant_chart.locus) | Six campaigns at the documented normalized positions, with the four documented quadrant captions. |
| [Requirement](https://mermaid.js.org/syntax/requirementDiagram.html) | Larger Example | [requirement_diagram.locus](requirement_diagram.locus) | The documented six requirement kinds, three elements, metadata, and eight typed relationships; long prose is shortened for the cards. |
| [Use case](https://mermaid.js.org/syntax/usecase.html) | Default theme, look and layout · Customer/Support | [use_case.locus](use_case.locus) | The documented two actors, Storefront/Fulfilment boundaries, three use cases, and Checkout includes Browse relationship. |
| [Git graph](https://mermaid.js.org/syntax/gitgraph.html) | Cherry-pick from a merge commit | [git_graph.locus](git_graph.locus) | The documented main/develop/release branches, ZERO/A/ONE/B/MERGE/TWO/THREE/C commits, and merge cherry-pick with parent B. |
| [C4](https://mermaid.js.org/syntax/c4.html) | C4 Container diagram | [c4_container.locus](c4_container.locus) | The documented customer, five containers, two external systems, and banking boundary; long descriptions and protocol labels are condensed. |
| [Mindmap](https://mermaid.js.org/syntax/mindmap.html) | Mindmap introduction | [mindmap.locus](mindmap.locus) | The documented Origins/Research/Tools hierarchy, including automatic creation and three uses. The decorative book icon is omitted. |
| [Timeline](https://mermaid.js.org/syntax/timeline.html) | Timeline of Industrial Revolution | [timeline.locus](timeline.locus) | The documented two eras and five Industry 1.0–5.0 periods, preserving their event categories. |
| [ZenUML style](https://mermaid.js.org/syntax/zenuml.html) | Try/Catch/Finally (Break) | [zenuml.locus](zenuml.locus) | The documented Consumer/API/BookingService participants, four messages, and try/catch/finally fragments. |
| [Sankey](https://mermaid.js.org/syntax/sankey.html) | Node labels / Node spacing example | [sankey.locus](sankey.locus) | The documented Electricity grid flows to homes (113.726), Industry (342.165), and Losses (56.691), with proportional port thicknesses. |
| [XY chart](https://mermaid.js.org/syntax/xyChart.html) | Sales Revenue | [xy_chart.locus](xy_chart.locus) | The documented twelve monthly values, with a shared bar/line series and the documented USD 4000–11000 vertical range. |
| [Block diagram](https://mermaid.js.org/syntax/block.html) | Introduction to Block Diagrams | [block_diagram.locus](block_diagram.locus) | The documented DB, nested A/B/C blocks with a wider B, and D output with two incoming connections. |
| [Packet](https://mermaid.js.org/syntax/packet.html) | TCP Packet | [packet_diagram.locus](packet_diagram.locus) | The documented TCP fields, six one-bit flags, options, and 64-bit data region; U/A/P/R/S/F abbreviate the flag names. |
| [Kanban](https://mermaid.js.org/syntax/kanban.html) | Complete Kanban example | [kanban_board.locus](kanban_board.locus) | The documented six columns and ten cards, including ticket/assignee/priority metadata. Long demonstration filler titles are abbreviated. |
| [Architecture](https://mermaid.js.org/syntax/architecture.html) | Aligning services · sources/storage/output | [architecture.locus](architecture.locus) | The documented nine services in three groups and six connections; service icons use labeled shapes. |
| [Radar](https://mermaid.js.org/syntax/radar.html) | Grades | [radar_chart.locus](radar_chart.locus) | The documented Alice/Bob scores over Math, Science, English, History, Geography, and Art on a 0–100 scale. |
| [Event modeling](https://mermaid.js.org/syntax/eventmodeling.html) | Reference events and processors | [event_modeling.locus](event_modeling.locus) | The documented CartUI/AddItem/ItemAdded sequence and external inventory event → processor → command → cart inventory event chain. |
| [Treemap](https://mermaid.js.org/syntax/treemap.html) | Products | [treemap.locus](treemap.locus) | The documented Electronics/Clothing hierarchy and five values (50, 30, 20, 40, 40), represented by an explicit area-preserving tiling. |
| [Venn](https://mermaid.js.org/syntax/venn.html) | What makes a good feature | [venn_diagram.locus](venn_diagram.locus) | The documented Desirable/Feasible/Viable sets, their three pairwise intersections, and the central Ship it intersection. |
| [Ishikawa](https://mermaid.js.org/syntax/ishikawa.html) | Blurry Photo | [ishikawa.locus](ishikawa.locus) | The documented Process/User/Equipment/Environment categories; lens and sensor subtrees are grouped into multiline labels to keep the figure compact. |
| [Wardley](https://mermaid.js.org/syntax/wardley.html) | Tea Shop Value Chain | [wardley_map.locus](wardley_map.locus) | The documented six components, five dependencies, two evolution targets, and standardising-power note. Coordinates preserve visibility/evolution order. |
| [Cynefin](https://mermaid.js.org/syntax/cynefin.html) | Incident Response | [cynefin.locus](cynefin.locus) | The documented practices in the four domains and the Unknown failure mode item in the central confusion domain. |
| [Tree view](https://mermaid.js.org/syntax/treeView.html) | Nested packages/mermaid/parser example | [tree_view.locus](tree_view.locus) | The documented packages/mermaid/parser hierarchy and source files, with a project root added to anchor the single figure. |

## Representation notes

- **Pie:** values match the adopted-pets example. Filled sectors approximate
  circular arcs with chords of at most five degrees.
- **Gantt:** the reference dates are expressed as elapsed calendar days from
  2014-01-01; this example does not require weekend exclusions.
- **Packet:** bit widths match the TCP example. Single-bit flags use the short
  labels U/A/P/R/S/F, with their full names in a legend. The data region is shown
  as two 32-bit rows.
- **Radar, quadrant, XY, Sankey, treemap, Wardley:** numeric values and normalized
  positions come from the selected examples. The treemap uses an explicit
  area-preserving tiling. Wardley input pairs are visibility/evolution, mapped
  to vertical/horizontal axes respectively.
- **Venn and Cynefin:** region sizes are schematic.
- **Class, ER, requirements, C4, use case:** relationships and stereotypes are
  drawn explicitly; the files do not implement domain-specific model validation.
- **Architecture, mindmap, tree view:** native labeled shapes replace icon packs.
- **Sequence and ZenUML:** the existing checkout remains the sequence figure.
  ZenUML follows the documented four-message try/catch/finally example.

## Validate and render

From the repository root:

```sh
python3 scripts/check_examples.py
cargo run --release -p locus-cli -- check examples/demonstrations/*.locus
cargo run --release -p locus-cli -- render examples/demonstrations/class_diagram.locus \
  -o target/class_diagram.svg
cargo test --release -p locus-render --test corpus_acceptance --all-features \
  all_demonstrations_render_with_shared_measurements
```

The source audit checks declarations and references. Native checks solve the
relations and execute assertions. Corpus acceptance checks all 42 demonstration
figures with shared bundled-font measurements and SVG/TikZ/PNG backend parity.
Normal checks may report incomplete layout-preference settling, so visual review
remains part of authoring.

To export all previews while running acceptance:

```sh
LOCUS_CORPUS_OUTPUT_DIR="$PWD/target/demonstration-preview" \
  cargo test --release -p locus-render --test corpus_acceptance --all-features \
  all_demonstrations_render_with_shared_measurements
```

Outputs appear under `target/demonstration-preview/demonstrations/`.
