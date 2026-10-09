// view! builds one nested tuple type per level of markup; a large screen nests past rustc's default limit of 128 in release builds.
#![recursion_limit = "512"]
