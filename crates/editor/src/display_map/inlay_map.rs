#![allow(unused)]
// TODO kb

use std::{
    ops::{Add, AddAssign, Range, Sub},
    sync::atomic::{self, AtomicUsize},
};

use crate::MultiBufferSnapshot;

use super::{
    suggestion_map::{
        SuggestionBufferRows, SuggestionChunks, SuggestionEdit, SuggestionOffset, SuggestionPoint,
        SuggestionSnapshot,
    },
    TextHighlights,
};
use collections::HashMap;
use gpui::fonts::HighlightStyle;
use language::{Chunk, Edit, Point, Rope, TextSummary};
use parking_lot::Mutex;
use project::InlayHint;
use rand::Rng;
use sum_tree::{Bias, SumTree};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InlayHintId(usize);

pub struct InlayMap {
    snapshot: Mutex<InlaySnapshot>,
    next_hint_id: AtomicUsize,
    inlay_hints: HashMap<InlayHintId, InlayHintToRender>,
}

#[derive(Clone)]
pub struct InlaySnapshot {
    // TODO kb merge these two together?
    pub suggestion_snapshot: SuggestionSnapshot,
    transforms: SumTree<Transform>,
    pub version: usize,
}

#[derive(Clone)]
struct Transform {
    input: TextSummary,
    output: TextSummary,
}

impl sum_tree::Item for Transform {
    type Summary = TextSummary;

    fn summary(&self) -> Self::Summary {
        self.output.clone()
    }
}

pub type InlayEdit = Edit<InlayOffset>;

#[derive(Copy, Clone, Debug, Default, Eq, Ord, PartialOrd, PartialEq)]
pub struct InlayOffset(pub usize);

impl Add for InlayOffset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for InlayOffset {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl AddAssign for InlayOffset {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, Ord, PartialOrd, PartialEq)]
pub struct InlayPoint(pub Point);

#[derive(Clone)]
pub struct InlayBufferRows<'a> {
    suggestion_rows: SuggestionBufferRows<'a>,
}

pub struct InlayChunks<'a> {
    suggestion_chunks: SuggestionChunks<'a>,
}

#[derive(Debug, Clone)]
pub struct InlayHintToRender {
    pub(super) position: InlayPoint,
    pub(super) text: Rope,
}

impl<'a> Iterator for InlayChunks<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.suggestion_chunks.next()
    }
}

impl<'a> Iterator for InlayBufferRows<'a> {
    type Item = Option<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        self.suggestion_rows.next()
    }
}

impl InlayPoint {
    pub fn new(row: u32, column: u32) -> Self {
        Self(Point::new(row, column))
    }

    pub fn row(self) -> u32 {
        self.0.row
    }

    pub fn column(self) -> u32 {
        self.0.column
    }
}

impl InlayMap {
    pub fn new(suggestion_snapshot: SuggestionSnapshot) -> (Self, InlaySnapshot) {
        let snapshot = InlaySnapshot {
            suggestion_snapshot: suggestion_snapshot.clone(),
            version: 0,
            transforms: SumTree::new(),
        };

        (
            Self {
                snapshot: Mutex::new(snapshot.clone()),
                next_hint_id: AtomicUsize::new(0),
                inlay_hints: HashMap::default(),
            },
            snapshot,
        )
    }

    pub fn sync(
        &self,
        suggestion_snapshot: SuggestionSnapshot,
        suggestion_edits: Vec<SuggestionEdit>,
    ) -> (InlaySnapshot, Vec<InlayEdit>) {
        let mut snapshot = self.snapshot.lock();

        if snapshot.suggestion_snapshot.version != suggestion_snapshot.version {
            snapshot.version += 1;
        }

        let mut inlay_edits = Vec::new();

        for suggestion_edit in suggestion_edits {
            let old = suggestion_edit.old;
            let new = suggestion_edit.new;
            // TODO kb copied from suggestion_map
            inlay_edits.push(InlayEdit {
                old: InlayOffset(old.start.0)..InlayOffset(old.end.0),
                new: InlayOffset(old.start.0)..InlayOffset(new.end.0),
            })
        }

        snapshot.suggestion_snapshot = suggestion_snapshot;

        (snapshot.clone(), inlay_edits)
    }

    // TODO kb replace set_inlay_hints with this
    pub fn splice(
        &mut self,
        to_remove: Vec<InlayHintId>,
        to_insert: Vec<InlayHintToRender>,
    ) -> Vec<InlayHintId> {
        // Order removals and insertions by position.
        // let anchors;

        // Remove and insert inlays in a single traversal across the tree.
        todo!("TODO kb")
    }

    pub fn set_inlay_hints(&mut self, new_hints: Vec<InlayHintToRender>) {
        // TODO kb reuse ids for hints that did not change and similar things
        self.inlay_hints = new_hints
            .into_iter()
            .map(|hint| {
                (
                    InlayHintId(self.next_hint_id.fetch_add(1, atomic::Ordering::SeqCst)),
                    hint,
                )
            })
            .collect();
    }
}

impl InlaySnapshot {
    pub fn buffer_snapshot(&self) -> &MultiBufferSnapshot {
        // TODO kb copied from suggestion_map
        self.suggestion_snapshot.buffer_snapshot()
    }

    pub fn to_point(&self, offset: InlayOffset) -> InlayPoint {
        // TODO kb copied from suggestion_map
        self.to_inlay_point(
            self.suggestion_snapshot
                .to_point(super::suggestion_map::SuggestionOffset(offset.0)),
        )
    }

    pub fn max_point(&self) -> InlayPoint {
        // TODO kb copied from suggestion_map
        self.to_inlay_point(self.suggestion_snapshot.max_point())
    }

    pub fn to_offset(&self, point: InlayPoint) -> InlayOffset {
        // TODO kb copied from suggestion_map
        InlayOffset(
            self.suggestion_snapshot
                .to_offset(self.to_suggestion_point(point, Bias::Left))
                .0,
        )
    }

    pub fn chars_at(&self, start: InlayPoint) -> impl '_ + Iterator<Item = char> {
        self.suggestion_snapshot
            .chars_at(self.to_suggestion_point(start, Bias::Left))
    }

    // TODO kb what to do with bias?
    pub fn to_suggestion_point(&self, point: InlayPoint, _: Bias) -> SuggestionPoint {
        SuggestionPoint(point.0)
    }

    pub fn to_inlay_point(&self, point: SuggestionPoint) -> InlayPoint {
        InlayPoint(point.0)
    }

    pub fn clip_point(&self, point: InlayPoint, bias: Bias) -> InlayPoint {
        // TODO kb copied from suggestion_map
        self.to_inlay_point(
            self.suggestion_snapshot
                .clip_point(self.to_suggestion_point(point, bias), bias),
        )
    }

    pub fn text_summary_for_range(&self, range: Range<InlayPoint>) -> TextSummary {
        // TODO kb copied from suggestion_map
        self.suggestion_snapshot.text_summary_for_range(
            self.to_suggestion_point(range.start, Bias::Left)
                ..self.to_suggestion_point(range.end, Bias::Left),
        )
    }

    pub fn buffer_rows<'a>(&'a self, row: u32) -> InlayBufferRows<'a> {
        InlayBufferRows {
            suggestion_rows: self.suggestion_snapshot.buffer_rows(row),
        }
    }

    pub fn line_len(&self, row: u32) -> u32 {
        // TODO kb copied from suggestion_map
        self.suggestion_snapshot.line_len(row)
    }

    pub fn chunks<'a>(
        &'a self,
        range: Range<InlayOffset>,
        language_aware: bool,
        text_highlights: Option<&'a TextHighlights>,
        suggestion_highlight: Option<HighlightStyle>,
    ) -> InlayChunks<'a> {
        // TODO kb copied from suggestion_map
        InlayChunks {
            suggestion_chunks: self.suggestion_snapshot.chunks(
                SuggestionOffset(range.start.0)..SuggestionOffset(range.end.0),
                language_aware,
                text_highlights,
                suggestion_highlight,
            ),
        }
    }

    #[cfg(test)]
    pub fn text(&self) -> String {
        // TODO kb copied from suggestion_map
        self.suggestion_snapshot.text()
    }
}
