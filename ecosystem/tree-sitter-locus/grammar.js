/// <reference types="tree-sitter-cli/dsl" />
// This grammar recognizes the current, development-only file format.
// Arity and type validation remain the responsibility of locus-syntax and the
// language server.
module.exports = grammar({
  name: "locus",

  extras: $ => [/[ \t]/, $.comment],
  word: $ => $.identifier,

  rules: {
    source_file: $ => seq(
      repeat(choice($._newline, seq($._statement, $._newline))),
      optional($._statement),
    ),

    _statement: $ => choice(
      $.header,
      $.meta_statement,
      $.style_statement,
      $.class_declaration,
      $.object_declaration,
      $.relation_statement,
      $.assert_statement,
    ),

    header: _ => seq("locus", "0", "beta", "5"),
    meta_statement: $ => seq("meta", field("key", $.identifier), "=", field("value", $.string)),
    style_statement: $ => seq("style", field("name", $.identifier)),
    class_declaration: $ => seq("class", field("name", $.identifier), $.properties),

    object_declaration: $ => seq(
      field("kind", $.kind), field("name", $.identifier), "=",
      field("value", choice($.free_expression, $._expression)),
      optional($.where_clause), optional($.attribute), optional($.properties),
    ),
    kind: _ => choice(
      "point", "num", "line", "ray", "segment", "circle", "arc", "ellipse",
      "bspline", "polygon", "rect", "rounded_rect", "diamond", "label", "arrow", "tick",
      "right_angle", "text_block",
    ),
    free_expression: $ => seq("free", $._expression),
    where_clause: $ => seq("where", "{", commaSeparated1($._expression), "}"),
    attribute: $ => seq("[", field("name", $.identifier), "]"),
    properties: $ => seq("{", optional(commaSeparated1($.property)), "}"),
    property: $ => seq(
      field("name", $.identifier), ":",
      field("value", choice($.arrow_head, $.hex_color, $._expression)),
    ),
    arrow_head: _ => choice("->", "<->", "-"),
    hex_color: _ => /#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?/,

    relation_statement: $ => seq("rel", $._expression),
    assert_statement: $ => seq("assert", $._expression, optional($.tolerance)),
    tolerance: $ => seq("tol", $.number),

    _expression: $ => choice(
      $.identifier, $.number, $.boolean, $.string, $.tex,
      $.call_expression, $.member_expression, $.group_expression,
      $.unary_expression, $.binary_expression,
    ),
    call_expression: $ => seq(field("function", $.identifier), $.arguments),
    arguments: $ => seq("(", optional(commaSeparated1($._expression)), ")"),
    member_expression: $ => prec.left(8, seq(
      field("object", $._expression), ".", field("property", $.identifier),
      optional($.arguments),
    )),
    group_expression: $ => seq("(", $._expression, ")"),
    unary_expression: $ => choice(
      prec.right(6, seq(field("operator", "-"), field("operand", $._expression))),
      prec.right(3, seq(field("operator", "not"), field("operand", $._expression))),
    ),
    binary_expression: $ => choice(
      ...[
        [1, "or"], [2, "and"],
        [3, "=="], [3, "<"], [3, ">"], [3, "<="], [3, ">="],
        [4, "+"], [4, "-"], [5, "*"], [5, "/"],
      ].map(([precedence, operator]) => prec.left(precedence, seq(
        field("left", $._expression), field("operator", operator),
        field("right", $._expression),
      ))),
    ),

    number: $ => seq(
      /([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][+-]?[0-9]+)?/,
      optional($.unit),
    ),
    unit: _ => token.immediate(choice("deg", "rad")),
    boolean: _ => choice("true", "false"),
    identifier: _ => /[A-Za-z][A-Za-z0-9_']*/,

    // Immediate tokens keep whitespace and // inside literals as text.
    string: $ => seq('"', repeat(choice($.string_content, $.escape_sequence, $.inline_tex)), token.immediate('"')),
    string_content: _ => token.immediate(/[^"\\$\r\n]+/),
    escape_sequence: _ => token.immediate(/\\["\\$n]/),
    inline_tex: $ => seq(token.immediate("$"), repeat(choice($.tex_content, $.tex_escape)), token.immediate("$")),
    tex: $ => seq("$", repeat(choice($.tex_content, $.tex_escape)), token.immediate("$")),
    tex_content: _ => token.immediate(/[^$\\\r\n]+/),
    tex_escape: _ => token.immediate(/\\[^\r\n]/),
    comment: _ => token(seq("//", /[^\r\n]*/)),
    _newline: _ => /\r\n|\n|\r/,
  },
});

function commaSeparated1(rule) {
  return seq(rule, repeat(seq(",", rule)));
}
