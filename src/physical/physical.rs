// Copyright 2020 The OctoSQL Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::hash::Hash;
use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;

pub const BATCH_SIZE: usize = 8192;
pub const RETRACTIONS_FIELD: &str = "retraction";

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum Identifier {
    SimpleIdentifier(String),
    NamespacedIdentifier(String, String),
}

impl ToString for Identifier {
    #[inline]
    fn to_string(&self) -> String {
        match self {
            Identifier::SimpleIdentifier(id) => {
                id.clone()
            }
            Identifier::NamespacedIdentifier(namespace, id) => {
                let mut output = namespace.clone();
                output.push_str(".");
                output.push_str(id);
                output
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScalarValue {
    Null,
    Boolean(bool),
    Float32(f32),
    Float64(f64),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Utf8(String),
    Struct(Vec<ScalarValue>),
}

impl ScalarValue {
    pub fn data_type(&self) -> DataType {
        match self {
            ScalarValue::Null => DataType::Null,
            ScalarValue::Boolean(_) => DataType::Boolean,
            ScalarValue::Float32(_) => DataType::Float32,
            ScalarValue::Float64(_) => DataType::Float64,
            ScalarValue::Int8(_) => DataType::Int8,
            ScalarValue::Int16(_) => DataType::Int16,
            ScalarValue::Int32(_) => DataType::Int32,
            ScalarValue::Int64(_) => DataType::Int64,
            ScalarValue::UInt8(_) => DataType::UInt8,
            ScalarValue::UInt16(_) => DataType::UInt16,
            ScalarValue::UInt32(_) => DataType::UInt32,
            ScalarValue::UInt64(_) => DataType::UInt64,
            ScalarValue::Utf8(_) => DataType::Utf8,
            ScalarValue::Struct(_) => /*DataType::Struct*/ unimplemented!(),
        }
    }
}

impl Hash for ScalarValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ScalarValue::Null => (),
            ScalarValue::Boolean(x) => x.hash(state),
            ScalarValue::Float32(_x) => unimplemented!(),
            ScalarValue::Float64(_x) => unimplemented!(),
            ScalarValue::Int8(x) => x.hash(state),
            ScalarValue::Int16(x) => x.hash(state),
            ScalarValue::Int32(x) => x.hash(state),
            ScalarValue::Int64(x) => x.hash(state),
            ScalarValue::UInt8(x) => x.hash(state),
            ScalarValue::UInt16(x) => x.hash(state),
            ScalarValue::UInt32(x) => x.hash(state),
            ScalarValue::UInt64(x) => x.hash(state),
            ScalarValue::Utf8(x) => x.hash(state),
            ScalarValue::Struct(x) => x.hash(state),
        }
    }
}

impl Eq for ScalarValue {}

pub struct ProduceContext {}

pub trait SchemaContext {
    fn field_with_name(&self, name: &str) -> Result<&Field, Error>;
}

pub struct EmptySchemaContext {
}

impl SchemaContext for EmptySchemaContext {
    fn field_with_name(&self, _name: &str) -> Result<&Field, Error> {
        Err(Error::Unexpected)
    }
}

pub struct SchemaContextWithSchema {
    pub previous: Arc<dyn SchemaContext>,
    pub schema: Arc<Schema>,
}

impl SchemaContext for SchemaContextWithSchema {
    fn field_with_name(&self, name: &str) -> Result<&Field, Error> {
        match self.schema.field_with_name(name) {
            Ok(field) => Ok(field),
            Err(arrow_err) => {
                match self.previous.field_with_name(name) {
                    Ok(field) => Ok(field),
                    Err(err) => Err(Error::Wrapped(format!("{}", arrow_err), Box::new(err.into()))),
                }
            },
        }
    }
}

// TODO: Fixme struct field visibility.
pub struct VariableContext {
    pub previous: Option<Arc<VariableContext>>,
    pub schema: Arc<Schema>,
    pub variables: Vec<ScalarValue>,
}

impl SchemaContext for VariableContext {
    fn field_with_name(&self, name: &str) -> Result<&Field, Error> {
        match self.schema.field_with_name(name) {
            Ok(field) => Ok(field),
            Err(arrow_err) => {
                match &self.previous {
                    None => Err(Error::Unexpected),
                    Some(previous) => match previous.field_with_name(name) {
                        Ok(field) => Ok(field),
                        Err(err) => Err(Error::Wrapped(format!("{}", arrow_err), Box::new(err.into()))),
                    },
                }
            },
        }
    }
}

pub struct ExecutionContext {
    pub variable_context: Arc<VariableContext>,
}

impl Clone for ExecutionContext {
    fn clone(&self) -> Self {
        ExecutionContext {
            variable_context: self.variable_context.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    ArrowError(arrow::error::ArrowError),
    Unexpected,
    Wrapped(String, Box<Error>),
    BadInput(String),
}

impl From<arrow::error::ArrowError> for Error {
    fn from(err: ArrowError) -> Self {
        Error::ArrowError(err)
    }
}

pub type ProduceFn<'a> = &'a mut dyn FnMut(&ProduceContext, RecordBatch) -> Result<(), Error>;
pub type MetaSendFn<'a> = &'a mut dyn FnMut(&ProduceContext, MetadataMessage) -> Result<(), Error>;

pub fn noop_meta_send(_ctx: &ProduceContext, _msg: MetadataMessage) -> Result<(), Error> {
    Ok(())
}

pub enum MetadataMessage {
    EndOfStream,
}

pub trait Node: Send + Sync {
    fn schema(&self, schema_context: Arc<dyn SchemaContext>) -> Result<Arc<Schema>, Error>;
    fn run(
        &self,
        ctx: &ExecutionContext,
        produce: ProduceFn,
        meta_send: MetaSendFn,
    ) -> Result<(), Error>;
}
