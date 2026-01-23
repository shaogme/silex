// Marker traits and types for HTML tags
// This file defines the type-safe markers used by TypedElement<T>

/// Root trait for all tag markers
pub trait Tag {}

// --- Group Traits (corresponding to props groups) ---

/// Tags that support form attributes (value, checked, type, etc.)
pub trait FormTag: Tag {}

/// Tags that support label attributes (for)
pub trait LabelTag: Tag {}

/// Tags that support anchor attributes (href, target, rel)
pub trait AnchorTag: Tag {}

/// Tags that support media attributes (src, alt, width, height)
pub trait MediaTag: Tag {}

/// Tags that can contain text content
pub trait TextTag: Tag {}

/// Tags that support the 'open' attribute (dialog, details)
pub trait OpenTag: Tag {}

/// Tags that are table cells (td, th) supporting colspan, rowspan
pub trait TableCellTag: Tag {}

/// Tags that are table headers (th) supporting scope, abbr
pub trait TableHeaderTag: Tag {}

// --- Tag Markers ---

// --- Tag Markers (Empty in Core) ---

// 6. SVG Tags Marker (Trait only)
pub trait SvgTag: Tag {}
