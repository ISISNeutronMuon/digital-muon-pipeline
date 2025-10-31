//! This module implements the traits to extend the hdf5 [Group] type to provide robust, conventient helper methods.
use super::{
    DatasetExt, GroupExt, HasAttributesExt,
    error::{ConvertResult, NexusHDF5Error, NexusHDF5Result},
};
use hdf5::{
    Attribute, Dataset, DatasetBuilderEmpty, Group, H5Type, SimpleExtents,
    types::{FloatSize, IntSize, TypeDescriptor, VarLenArray, VarLenUnicode},
};

impl HasAttributesExt for Group {
    fn add_attribute<T: H5Type>(&self, attr: &str) -> NexusHDF5Result<Attribute> {
        let attr = self.new_attr::<T>().create(attr).err_group(self)?;
        Ok(attr)
    }

    /// This should be a provided method in the trait?
    fn add_string_attribute(&self, attr: &str) -> NexusHDF5Result<Attribute> {
        self.add_attribute::<VarLenUnicode>(attr)
    }

    fn add_constant_string_attribute(&self, attr: &str, value: &str) -> NexusHDF5Result<Attribute> {
        let attr = self.add_string_attribute(attr)?;
        attr.write_scalar(&value.parse::<VarLenUnicode>().err_group(self)?)
            .err_group(self)?;
        Ok(attr)
    }

    fn get_attribute(&self, attr: &str) -> NexusHDF5Result<Attribute> {
        self.attr(attr).err_group(self)
    }
}

/// Creates a hdf5 [DatasetBuilderEmpty] object with the appropriate type specified by `type_descriptor`.
/// This is only used by [create_dynamic_resizable_empty_dataset].
///
/// [create_dynamic_resizable_empty_dataset]: Group::create_dynamic_resizable_empty_dataset()
fn get_dataset_builder(
    type_descriptor: &TypeDescriptor,
    parent: &Group,
) -> NexusHDF5Result<DatasetBuilderEmpty> {
    Ok(match type_descriptor {
        TypeDescriptor::Integer(sz) => match sz {
            IntSize::U1 => parent.new_dataset::<i8>(),
            IntSize::U2 => parent.new_dataset::<i16>(),
            IntSize::U4 => parent.new_dataset::<i32>(),
            IntSize::U8 => parent.new_dataset::<i64>(),
        },
        TypeDescriptor::Unsigned(sz) => match sz {
            IntSize::U1 => parent.new_dataset::<u8>(),
            IntSize::U2 => parent.new_dataset::<u16>(),
            IntSize::U4 => parent.new_dataset::<u32>(),
            IntSize::U8 => parent.new_dataset::<u64>(),
        },
        TypeDescriptor::Float(sz) => match sz {
            FloatSize::U4 => parent.new_dataset::<f32>(),
            FloatSize::U8 => parent.new_dataset::<f64>(),
        },
        TypeDescriptor::VarLenUnicode => parent.new_dataset::<VarLenUnicode>(),
        TypeDescriptor::VarLenArray(inner_type_descriptor) => {
            match inner_type_descriptor.to_packed_repr() {
                TypeDescriptor::Integer(sz) => match sz {
                    IntSize::U1 => parent.new_dataset::<VarLenArray<i8>>(),
                    IntSize::U2 => parent.new_dataset::<VarLenArray<i16>>(),
                    IntSize::U4 => parent.new_dataset::<VarLenArray<i32>>(),
                    IntSize::U8 => parent.new_dataset::<VarLenArray<i64>>(),
                },
                TypeDescriptor::Unsigned(sz) => match sz {
                    IntSize::U1 => parent.new_dataset::<VarLenArray<u8>>(),
                    IntSize::U2 => parent.new_dataset::<VarLenArray<u16>>(),
                    IntSize::U4 => parent.new_dataset::<VarLenArray<u32>>(),
                    IntSize::U8 => parent.new_dataset::<VarLenArray<u64>>(),
                },
                TypeDescriptor::Float(sz) => match sz {
                    FloatSize::U4 => parent.new_dataset::<VarLenArray<f32>>(),
                    FloatSize::U8 => parent.new_dataset::<VarLenArray<f64>>(),
                },
                _ => {
                    return Err(NexusHDF5Error::InvalidHDF5Type {
                        error: type_descriptor.clone(),
                        hdf5_path: None,
                    });
                }
            }
        }
        _ => {
            return Err(NexusHDF5Error::InvalidHDF5Type {
                error: type_descriptor.clone(),
                hdf5_path: None,
            });
        }
    })
}

impl GroupExt for Group {
    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn add_new_group(&self, name: &str, class: &str) -> NexusHDF5Result<Group> {
        let group = self.create_group(name).err_group(self)?;
        group.set_nx_class(class)?;
        Ok(group)
    }

    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn set_nx_class(&self, class: &str) -> NexusHDF5Result<()> {
        self.add_constant_string_attribute("NX_class", class)?;
        Ok(())
    }

    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn create_scalar_dataset<T: H5Type>(&self, name: &str) -> NexusHDF5Result<Dataset> {
        self.new_dataset::<T>().create(name).err_group(self)
    }

    /// This should be a provided function of the trait.
    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn create_string_dataset(&self, name: &str) -> NexusHDF5Result<Dataset> {
        self.create_scalar_dataset::<VarLenUnicode>(name)
    }

    /// This should be a provided function of the trait.
    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn create_constant_scalar_dataset<T: H5Type>(
        &self,
        name: &str,
        value: &T,
    ) -> NexusHDF5Result<Dataset> {
        let dataset = self.create_scalar_dataset::<T>(name)?;
        dataset.set_scalar(value)?;
        Ok(dataset)
    }

    /// This should be a provided function of the trait.
    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn create_constant_string_dataset(&self, name: &str, value: &str) -> NexusHDF5Result<Dataset> {
        let dataset = self.create_scalar_dataset::<VarLenUnicode>(name)?;
        dataset.set_string(value)?;
        Ok(dataset)
    }

    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn create_resizable_empty_dataset<T: H5Type>(
        &self,
        name: &str,
        chunk_size: usize,
    ) -> NexusHDF5Result<Dataset> {
        self.new_dataset::<T>()
            .shape(SimpleExtents::resizable(vec![0]))
            .chunk(vec![chunk_size])
            .create(name)
            .err_group(self)
    }

    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn create_dynamic_resizable_empty_dataset(
        &self,
        name: &str,
        type_descriptor: &TypeDescriptor,
        chunk_size: usize,
    ) -> NexusHDF5Result<Dataset> {
        get_dataset_builder(type_descriptor, self)
            .err_group(self)?
            .shape(SimpleExtents::resizable(vec![0]))
            .chunk(chunk_size)
            .create(name)
            .err_group(self)
    }

    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn get_dataset(&self, name: &str) -> NexusHDF5Result<Dataset> {
        self.dataset(name).err_group(self)
    }

    #[cfg(test)]
    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn get_dataset_or_else<F>(&self, name: &str, f: F) -> NexusHDF5Result<Dataset>
    where
        F: Fn(&Group) -> NexusHDF5Result<Dataset>,
    {
        self.dataset(name).or_else(|_| f(self))
    }

    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn get_group(&self, name: &str) -> NexusHDF5Result<Group> {
        self.group(name).err_group(self)
    }

    #[cfg(test)]
    #[tracing::instrument(skip_all, level = "trace", err(level = "warn"))]
    fn get_group_or_create_new(&self, name: &str, class: &str) -> NexusHDF5Result<Group> {
        self.group(name)
            .or_else(|_| self.add_new_group(name, class))
    }
}
