# Unreleased

Nothing yet

# Version 1.4.0-alpha
## DO NOT USE UNLESS YOU KNOW WHAT YOU ARE DOING
Added colorscheme field to json and added realtime change monitoring on config. It is very very broken and in early stages now.

This release exists only for testers.

Default json file
```json
{
  "game_path": "<your game path>",
  "default_font_size": 18.0,
  "colorscheme": {
    "hyperlink_color": "rgb(247, 16, 52)",
    "error_fg_color": "rgb(255, 0, 0)",
    "warn_fg_color": "rgb(255, 143, 0)",
    "text": "rgb(140, 140, 140)",
    "border": "rgb(60, 60, 60)",
    "hovered_widget_color": "rgb(70, 70, 70)",
    "active_widget_color": "rgb(55, 55, 55)",
    "widget_text_color": "rgb(210, 210, 210)",
    "widget_active_text_color": "rgb(255, 255, 255)",
    "widget_hovered_text_color": "rgb(240, 240, 240)",
    "widget_color": "rgb(27, 27, 27)",
    "inactive_widget_color": "rgb(60, 60, 60)",
    "base_color": "rgb(27, 27, 27)",
    "code_bg_color": "rgb(10, 10, 10)",
    "extreme_bg_color": "rgb(10, 10, 10)",
    "selection_bg_fill": "rgba(242, 24, 13, 60)",
    "shadow_color": "rgba(0, 0, 0, 96)",
    "stroke_color": "rgb(148, 20, 40)",
    "selection_stoke_color": "rgb(0, 0, 0)",
    "faint_bg_color": "rgba(5, 5, 5, 0)",
    "widget_in_active_text_color": "rgb(180, 180, 180)"
  }
}
```