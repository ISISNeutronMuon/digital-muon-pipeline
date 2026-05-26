use std::collections::HashMap;

use digital_muon_common::Channel;

use crate::{analysis::SumWithSumOfSqrs, event::ChannelData};

pub(crate) struct AnalysisWithTrue {
    channel: Channel,
    num_frames: usize,
    num_false_positives: SumWithSumOfSqrs,
    num_false_negatives: SumWithSumOfSqrs,
}

impl AnalysisWithTrue {
    fn push(true_data: &ChannelData, data: ChannelData) {
        let mut temp = HashMap::<usize, Vec<usize>>::new();
    }
}
