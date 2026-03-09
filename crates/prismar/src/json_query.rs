use serde::{Deserialize, Serialize};

use crate::{
  BoolFilter, Condition, FieldFilter, JsonFilter, ModelFilter, NumberFilter,
  RelationFilterOp, StringFilter,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PrismaReadManyInput {
  #[serde(rename = "where", default)]
  pub r#where: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(transparent)]
pub struct PrismaWhereInput(pub serde_json::Value);

impl PrismaReadManyInput {
  pub fn to_args(self) -> Result<crate::ReadManyArgs, String> {
    Ok(crate::ReadManyArgs {
      r#where: self.r#where.map(parse_model_filter).transpose()?,
      order_by: None,
      pagination: None,
    })
  }
}

impl PrismaWhereInput {
  pub fn to_filter(self) -> Result<ModelFilter, String> {
    parse_model_filter(self.0)
  }
}

pub fn parse_model_filter(
  value: serde_json::Value,
) -> Result<ModelFilter, String> {
  let serde_json::Value::Object(map) = value else {
    return Err("where must be a JSON object".to_owned());
  };

  let mut filter = ModelFilter::empty();
  for (key, value) in map {
    match key.as_str() {
      "AND" => {
        let filters = parse_filter_list(value)?;
        filter.conditions.push(Condition::And(filters));
      }
      "OR" => {
        let filters = parse_filter_list(value)?;
        filter.conditions.push(Condition::Or(filters));
      }
      "NOT" => match value {
        serde_json::Value::Array(values) => {
          for value in values {
            filter
              .conditions
              .push(Condition::Not(Box::new(parse_model_filter(value)?)));
          }
        }
        other => {
          filter
            .conditions
            .push(Condition::Not(Box::new(parse_model_filter(other)?)));
        }
      },
      field => {
        append_field_condition(&mut filter, field, value)?;
      }
    }
  }

  Ok(filter)
}

fn parse_filter_list(
  value: serde_json::Value,
) -> Result<Vec<ModelFilter>, String> {
  match value {
    serde_json::Value::Array(values) => {
      values.into_iter().map(parse_model_filter).collect()
    }
    serde_json::Value::Object(_) => Ok(vec![parse_model_filter(value)?]),
    _ => {
      Err("logical operators expect an object or array of objects".to_owned())
    }
  }
}

fn append_field_condition(
  filter: &mut ModelFilter,
  field: &str,
  value: serde_json::Value,
) -> Result<(), String> {
  match value {
    serde_json::Value::Null => {
      filter
        .conditions
        .push(Condition::Predicate(crate::Predicate {
          field: field.to_owned(),
          filter: FieldFilter::Null,
        }));
    }
    serde_json::Value::Bool(value) => {
      filter
        .conditions
        .push(Condition::Predicate(crate::Predicate {
          field: field.to_owned(),
          filter: FieldFilter::Bool(BoolFilter::Equals(value)),
        }));
    }
    serde_json::Value::Number(value) => {
      let Some(value) = value.as_f64() else {
        return Err(format!("numeric value for {field} is not supported"));
      };
      filter
        .conditions
        .push(Condition::Predicate(crate::Predicate {
          field: field.to_owned(),
          filter: FieldFilter::Number(NumberFilter::Equals(value)),
        }));
    }
    serde_json::Value::String(value) => {
      filter
        .conditions
        .push(Condition::Predicate(crate::Predicate {
          field: field.to_owned(),
          filter: FieldFilter::String(StringFilter::Equals(value)),
        }));
    }
    serde_json::Value::Array(values) => {
      let all_strings = values.iter().all(|value| value.is_string());
      let all_numbers = values.iter().all(|value| value.is_number());
      if all_strings {
        filter
          .conditions
          .push(Condition::Predicate(crate::Predicate {
            field: field.to_owned(),
            filter: FieldFilter::String(StringFilter::In(
              values
                .into_iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect(),
            )),
          }));
      } else if all_numbers {
        filter
          .conditions
          .push(Condition::Predicate(crate::Predicate {
            field: field.to_owned(),
            filter: FieldFilter::Number(NumberFilter::In(
              values
                .into_iter()
                .filter_map(|value| value.as_f64())
                .collect(),
            )),
          }));
      } else {
        filter
          .conditions
          .push(Condition::Predicate(crate::Predicate {
            field: field.to_owned(),
            filter: FieldFilter::Json(JsonFilter::Equals(
              serde_json::Value::Array(values),
            )),
          }));
      }
    }
    serde_json::Value::Object(map) => {
      if let Some(value) = map.get("some") {
        filter
          .conditions
          .push(Condition::Relation(crate::RelationPredicate {
            field: field.to_owned(),
            op: RelationFilterOp::Some,
            filter: Box::new(parse_model_filter(value.clone())?),
          }));
        return Ok(());
      }
      if let Some(value) = map.get("every") {
        filter
          .conditions
          .push(Condition::Relation(crate::RelationPredicate {
            field: field.to_owned(),
            op: RelationFilterOp::Every,
            filter: Box::new(parse_model_filter(value.clone())?),
          }));
        return Ok(());
      }
      if let Some(value) = map.get("none") {
        filter
          .conditions
          .push(Condition::Relation(crate::RelationPredicate {
            field: field.to_owned(),
            op: RelationFilterOp::None,
            filter: Box::new(parse_model_filter(value.clone())?),
          }));
        return Ok(());
      }
      if let Some(value) = map.get("is") {
        filter
          .conditions
          .push(Condition::Relation(crate::RelationPredicate {
            field: field.to_owned(),
            op: RelationFilterOp::Is,
            filter: Box::new(parse_model_filter(value.clone())?),
          }));
        return Ok(());
      }
      if let Some(value) = map.get("isNot") {
        filter
          .conditions
          .push(Condition::Relation(crate::RelationPredicate {
            field: field.to_owned(),
            op: RelationFilterOp::IsNot,
            filter: Box::new(parse_model_filter(value.clone())?),
          }));
        return Ok(());
      }

      for (op, value) in map {
        let field_filter = match (op.as_str(), value) {
          ("equals", serde_json::Value::String(value)) => {
            FieldFilter::String(StringFilter::Equals(value))
          }
          ("equals", serde_json::Value::Number(value)) => {
            FieldFilter::Number(NumberFilter::Equals(
              value
                .as_f64()
                .ok_or_else(|| format!("invalid number for {field}"))?,
            ))
          }
          ("equals", serde_json::Value::Bool(value)) => {
            FieldFilter::Bool(BoolFilter::Equals(value))
          }
          ("equals", serde_json::Value::Null) => FieldFilter::Null,
          ("not", serde_json::Value::String(value)) => {
            FieldFilter::String(StringFilter::NotEquals(value))
          }
          ("not", serde_json::Value::Number(value)) => {
            FieldFilter::Number(NumberFilter::NotEquals(
              value
                .as_f64()
                .ok_or_else(|| format!("invalid number for {field}"))?,
            ))
          }
          ("not", serde_json::Value::Bool(value)) => {
            FieldFilter::Bool(BoolFilter::NotEquals(value))
          }
          ("in", serde_json::Value::Array(values))
            if values.iter().all(|value| value.is_string()) =>
          {
            FieldFilter::String(StringFilter::In(
              values
                .into_iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect(),
            ))
          }
          ("in", serde_json::Value::Array(values))
            if values.iter().all(|value| value.is_number()) =>
          {
            FieldFilter::Number(NumberFilter::In(
              values
                .into_iter()
                .filter_map(|value| value.as_f64())
                .collect(),
            ))
          }
          ("notIn", serde_json::Value::Array(values))
            if values.iter().all(|value| value.is_string()) =>
          {
            FieldFilter::String(StringFilter::NotIn(
              values
                .into_iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect(),
            ))
          }
          ("notIn", serde_json::Value::Array(values))
            if values.iter().all(|value| value.is_number()) =>
          {
            FieldFilter::Number(NumberFilter::NotIn(
              values
                .into_iter()
                .filter_map(|value| value.as_f64())
                .collect(),
            ))
          }
          ("gt", serde_json::Value::Number(value)) => {
            FieldFilter::Number(NumberFilter::GreaterThan(
              value
                .as_f64()
                .ok_or_else(|| format!("invalid number for {field}"))?,
            ))
          }
          ("gte", serde_json::Value::Number(value)) => {
            FieldFilter::Number(NumberFilter::GreaterThanOrEquals(
              value
                .as_f64()
                .ok_or_else(|| format!("invalid number for {field}"))?,
            ))
          }
          ("lt", serde_json::Value::Number(value)) => {
            FieldFilter::Number(NumberFilter::LessThan(
              value
                .as_f64()
                .ok_or_else(|| format!("invalid number for {field}"))?,
            ))
          }
          ("lte", serde_json::Value::Number(value)) => {
            FieldFilter::Number(NumberFilter::LessThanOrEquals(
              value
                .as_f64()
                .ok_or_else(|| format!("invalid number for {field}"))?,
            ))
          }
          ("contains", serde_json::Value::String(value)) => {
            FieldFilter::String(StringFilter::Contains(value))
          }
          ("notContains", serde_json::Value::String(value)) => {
            FieldFilter::String(StringFilter::NotContains(value))
          }
          ("startsWith", serde_json::Value::String(value)) => {
            FieldFilter::String(StringFilter::StartsWith(value))
          }
          ("endsWith", serde_json::Value::String(value)) => {
            FieldFilter::String(StringFilter::EndsWith(value))
          }
          (_, value) => FieldFilter::Json(JsonFilter::Equals(value)),
        };

        filter
          .conditions
          .push(Condition::Predicate(crate::Predicate {
            field: field.to_owned(),
            filter: field_filter,
          }));
      }
    }
  }
  Ok(())
}
