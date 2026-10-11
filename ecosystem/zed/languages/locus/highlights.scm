(identifier) @variable
(kind) @type
["locus" "beta" "meta" "style" "class" "fn" "group" "rel" "assert" "where" "free" "tol"] @keyword
["and" "or" "not"] @operator
["=" "==" "<" ">" "<=" ">=" "+" "-" "*" "/"] @operator
(boolean) @boolean
(number) @number
["0" "6"] @number
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
(meta_statement key: (identifier) @property)
(class_declaration name: (identifier) @type)
(function_declaration name: (identifier) @function)
(instance_declaration function: (identifier) @function)
(attribute name: (identifier) @attribute)
(style_statement name: (identifier) @attribute)

["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":" "."] @punctuation.delimiter
