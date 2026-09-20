(identifier) @variable
(kind) @type
["locus" "beta" "meta" "style" "class" "rel" "assert" "where" "free" "tol"] @keyword
["and" "or" "not"] @operator
["=" "==" "<" ">" "<=" ">=" "+" "-" "*" "/" "~"] @operator
(boolean) @boolean
(number) @number
["0" "4"] @number
(hex_color) @string.special
(arrow_head) @operator
(comment) @comment
(string) @string
(escape_sequence) @string.escape
(tex) @string.special
(inline_tex) @string.special
(tex_escape) @string.escape

(call_expression function: (identifier) @function)
(member_expression property: (identifier) @property)
(property name: (identifier) @property)
(meta_entry key: (identifier) @property)
(class_declaration name: (identifier) @type)
(attribute name: (identifier) @attribute)
(style_statement name: (identifier) @attribute)

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":" "."] @punctuation.delimiter
