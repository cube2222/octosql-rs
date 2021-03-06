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

use std::fs::File;
use std::sync::Arc;

use arrow::array::{ArrayRef, BooleanBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::json;
use arrow::record_batch::RecordBatch;

use crate::physical::physical::*;

pub struct JSONSource {
    path: String,
}

impl JSONSource {
    pub fn new(path: String) -> JSONSource {
        JSONSource { path }
    }
}

impl Node for JSONSource {
    fn schema(&self, _schema_context: Arc<dyn SchemaContext>) -> Result<Arc<Schema>, Error> {
        let file = File::open(self.path.as_str()).unwrap();
        let r = json::ReaderBuilder::new()
            .infer_schema(Some(10))
            .with_batch_size(BATCH_SIZE)
            .build(file)
            .unwrap();
        let mut fields = r.schema().fields().clone();
        fields.push(Field::new(RETRACTIONS_FIELD, DataType::Boolean, false));

        Ok(Arc::new(Schema::new(fields)))
    }

    fn run(
        &self,
        ctx: &ExecutionContext,
        produce: ProduceFn,
        _meta_send: MetaSendFn,
    ) -> Result<(), Error> {
        let file = File::open(self.path.as_str()).unwrap();
        let mut r = json::ReaderBuilder::new()
            .infer_schema(Some(10))
            .with_batch_size(BATCH_SIZE)
            .build(file)
            .unwrap();
        let mut retraction_array_builder = BooleanBuilder::new(BATCH_SIZE);
        for _i in 0..BATCH_SIZE {
            retraction_array_builder.append_value(false)?;
        }
        let retraction_array = Arc::new(retraction_array_builder.finish());
        let schema = self.schema(ctx.variable_context.clone())?;
        loop {
            let maybe_rec = r.next().unwrap();
            match maybe_rec {
                None => break,
                Some(rec) => {
                    let mut columns: Vec<ArrayRef> = rec.columns().iter().cloned().collect();
                    if columns[0].len() == BATCH_SIZE {
                        columns.push(retraction_array.clone() as ArrayRef)
                    } else {
                        let mut retraction_array_builder = BooleanBuilder::new(BATCH_SIZE);
                        for _i in 0..columns[0].len() {
                            retraction_array_builder.append_value(false)?;
                        }
                        let retraction_array = Arc::new(retraction_array_builder.finish());
                        columns.push(retraction_array as ArrayRef)
                    }
                    produce(
                        &ProduceContext {},
                        RecordBatch::try_new(schema.clone(), columns).unwrap(),
                    )?
                }
            };
        }
        Ok(())
    }
}
