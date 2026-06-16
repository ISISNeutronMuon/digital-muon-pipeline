mod cached_dataset;
mod channel;
mod digitiser;

use std::{fmt::Debug, str::FromStr};

pub(crate) use digitiser::{Hdf5Digitiser, HDF5Config};

fn extract_from_dataset_name<'a, T>(name: String, identifier: &'static str) -> Result<T, String>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    let group_name = name
        .split('/')
        .last()
        .unwrap()
        .split('_')
        .collect::<Vec<_>>();
    if group_name.len() < 1 {
        return Err("No underscore".into());
    }
    if group_name.get(0).unwrap() != &identifier {
        return Err(format!(
            "Wrong Identifier. Expected {}, got {}",
            identifier,
            group_name.get(0).unwrap()
        ));
    }
    Ok(group_name.get(1).unwrap().parse().unwrap())
}
