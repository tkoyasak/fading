; See <https://docs.helix-editor.com/guides/injection.html>.
; code block
(fenced_code_block
  (code_fence_content) @injection.shebang @injection.content
  (#set! injection.include-unnamed-children))

(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content
  (#set! injection.include-unnamed-children))

; metadata
((plus_metadata) @injection.content
  (#set! injection.language "toml")
  (#set! injection.include-unnamed-children))

; inline
((inline) @injection.content
  (#set! injection.language "markdown.inline")
  (#set! injection.include-unnamed-children))
