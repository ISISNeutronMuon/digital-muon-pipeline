use super::actions::SelectionModeOptions;
use chrono::Utc;
use rand::{RngExt, SeedableRng, seq::SliceRandom};
use std::collections::VecDeque;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum CacheError {
    #[error("Attempted to read from Empty Cache")]
    CacheEmpty,
    #[error("Attempted to read {0} form  Cache of size {1}")]
    CacheTooSmall(usize, usize),
}

pub(crate) trait SimulationEngineCache: Default + Extend<Self::Item> {
    type Item;

    fn extract_one(
        &mut self,
        selection_mode: SelectionModeOptions,
    ) -> Result<&Self::Item, CacheError>;
    fn extract(
        &mut self,
        selection_mode: SelectionModeOptions,
        amount: usize,
    ) -> Result<Vec<&Self::Item>, CacheError>;
    fn finish_one(&mut self, selection_mode: SelectionModeOptions) -> Result<(), CacheError>;
    fn finish(
        &mut self,
        selection_mode: SelectionModeOptions,
        amount: usize,
    ) -> Result<(), CacheError>;
}

impl<T> SimulationEngineCache for VecDeque<T> {
    type Item = T;

    fn extract_one(
        &mut self,
        selection_mode: SelectionModeOptions,
    ) -> Result<&Self::Item, CacheError> {
        match selection_mode {
            SelectionModeOptions::PopFront => self.front().ok_or(CacheError::CacheEmpty),
            SelectionModeOptions::ReplaceRandom => {
                let mut rng =
                    rand::rngs::StdRng::seed_from_u64(Utc::now().timestamp_subsec_nanos().into());
                self.get(rng.random_range(0..self.len()))
                    .ok_or(CacheError::CacheEmpty)
            }
        }
    }

    fn extract(
        &mut self,
        selection_mode: SelectionModeOptions,
        amount: usize,
    ) -> Result<Vec<&Self::Item>, CacheError> {
        let indices = match selection_mode {
            SelectionModeOptions::PopFront => self.iter().take(amount).collect(),
            SelectionModeOptions::ReplaceRandom => {
                let mut rng =
                    rand::rngs::StdRng::seed_from_u64(Utc::now().timestamp_subsec_nanos().into());
                let mut indices = (0..self.len()).collect::<Vec<_>>();
                let (random_indices, _) = indices.partial_shuffle(&mut rng, amount);
                random_indices
                    .iter()
                    .map(|i| self.get(*i))
                    .collect::<Option<_>>()
                    .ok_or(CacheError::CacheTooSmall(amount, self.len()))?
            }
        };
        Ok(indices)
    }

    fn finish_one(&mut self, selection_mode: SelectionModeOptions) -> Result<(), CacheError> {
        match selection_mode {
            SelectionModeOptions::PopFront => {
                self.pop_front().ok_or(CacheError::CacheEmpty)?;
            }
            SelectionModeOptions::ReplaceRandom => {}
        }
        Ok(())
    }

    fn finish(
        &mut self,
        selection_mode: SelectionModeOptions,
        amount: usize,
    ) -> Result<(), CacheError> {
        if self.len() < amount {
            return Err(CacheError::CacheTooSmall(amount, self.len()));
        }
        match selection_mode {
            SelectionModeOptions::PopFront => {
                self.drain(0..amount);
            }
            SelectionModeOptions::ReplaceRandom => (),
        };
        Ok(())
    }
}
