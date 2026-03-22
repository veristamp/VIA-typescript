Filtering
With Qdrant, you can set conditions when searching or retrieving points. For example, you can impose conditions on both the payload and the id of the point.

Setting additional conditions is important when it is impossible to express all the features of the object in the embedding. Examples include a variety of business requirements: stock availability, user location, or desired price range.

Related Content
A Complete Guide to Filtering in Vector Search	Developer advice on proper usage and advanced practices.
Filtering clauses
Qdrant allows you to combine conditions in clauses. Clauses are different logical operations, such as OR, AND, and NOT. Clauses can be recursively nested into each other so that you can reproduce an arbitrary boolean expression.

Let’s take a look at the clauses implemented in Qdrant.

Suppose we have a set of points with the following payload:

[
  { "id": 1, "city": "London", "color": "green" },
  { "id": 2, "city": "London", "color": "red" },
  { "id": 3, "city": "London", "color": "blue" },
  { "id": 4, "city": "Berlin", "color": "red" },
  { "id": 5, "city": "Moscow", "color": "green" },
  { "id": 6, "city": "Moscow", "color": "blue" }
]

Must
When using must, the clause becomes true only if every condition listed inside must is satisfied. In this sense, must is equivalent to the operator AND.

Example:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must": [
            { "key": "city", "match": { "value": "London" } },
            { "key": "color", "match": { "value": "red" } }
        ]
    }
    ...
}

Filtered points would be:

[{ "id": 2, "city": "London", "color": "red" }]

Should
When using should, the clause becomes true if at least one condition listed inside should is satisfied. In this sense, should is equivalent to the operator OR.

Example:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "should": [
            { "key": "city", "match": { "value": "London" } },
            { "key": "color", "match": { "value": "red" } }
        ]
    }
}

Filtered points would be:

[
  { "id": 1, "city": "London", "color": "green" },
  { "id": 2, "city": "London", "color": "red" },
  { "id": 3, "city": "London", "color": "blue" },
  { "id": 4, "city": "Berlin", "color": "red" }
]

Must Not
When using must_not, the clause becomes true if none of the conditions listed inside must_not is satisfied. In this sense, must_not is equivalent to the expression (NOT A) AND (NOT B) AND (NOT C).

Example:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must_not": [
            { "key": "city", "match": { "value": "London" } },
            { "key": "color", "match": { "value": "red" } }
        ]
    }
}

Filtered points would be:

[
  { "id": 5, "city": "Moscow", "color": "green" },
  { "id": 6, "city": "Moscow", "color": "blue" }
]

Clauses combination
It is also possible to use several clauses simultaneously:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must": [
            { "key": "city", "match": { "value": "London" } }
        ],
        "must_not": [
            { "key": "color", "match": { "value": "red" } }
        ]
    }
}

Filtered points would be:

[
  { "id": 1, "city": "London", "color": "green" },
  { "id": 3, "city": "London", "color": "blue" }
]

In this case, the conditions are combined by AND.

Also, the conditions could be recursively nested. Example:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must_not": [
            {
                "must": [
                    { "key": "city", "match": { "value": "London" } },
                    { "key": "color", "match": { "value": "red" } }
                ]
            }
        ]
    }
}

Filtered points would be:

[
  { "id": 1, "city": "London", "color": "green" },
  { "id": 3, "city": "London", "color": "blue" },
  { "id": 4, "city": "Berlin", "color": "red" },
  { "id": 5, "city": "Moscow", "color": "green" },
  { "id": 6, "city": "Moscow", "color": "blue" }
]

Filtering conditions
Different types of values in payload correspond to different kinds of queries that we can apply to them. Let’s look at the existing condition variants and what types of data they apply to.

Match
json
python
typescript
rust
java
csharp
go
{
  "key": "color",
  "match": {
    "value": "red"
  }
}

For the other types, the match condition will look exactly the same, except for the type used:

json
python
typescript
rust
java
csharp
go
{
  "key": "count",
  "match": {
    "value": 0
  }
}

The simplest kind of condition is one that checks if the stored value equals the given one. If several values are stored, at least one of them should match the condition. You can apply it to keyword, integer and bool payloads.

Match Any
Available as of v1.1.0

In case you want to check if the stored value is one of multiple values, you can use the Match Any condition. Match Any works as a logical OR for the given values. It can also be described as a IN operator.

You can apply it to keyword and integer payloads.

Example:

json
python
typescript
rust
java
csharp
go
{
  "key": "color",
  "match": {
    "any": ["black", "yellow"]
  }
}

In this example, the condition will be satisfied if the stored value is either black or yellow.

If the stored value is an array, it should have at least one value matching any of the given values. E.g. if the stored value is ["black", "green"], the condition will be satisfied, because "black" is in ["black", "yellow"].

Match Except
Available as of v1.2.0

In case you want to check if the stored value is not one of multiple values, you can use the Match Except condition. Match Except works as a logical NOR for the given values. It can also be described as a NOT IN operator.

You can apply it to keyword and integer payloads.

Example:

json
python
typescript
rust
java
csharp
go
{
  "key": "color",
  "match": {
    "except": ["black", "yellow"]
  }
}

In this example, the condition will be satisfied if the stored value is neither black nor yellow.

If the stored value is an array, it should have at least one value not matching any of the given values. E.g. if the stored value is ["black", "green"], the condition will be satisfied, because "green" does not match "black" nor "yellow".

Nested key
Available as of v1.1.0

Payloads being arbitrary JSON object, it is likely that you will need to filter on a nested field.

For convenience, we use a syntax similar to what can be found in the Jq project.

Suppose we have a set of points with the following payload:

[
  {
    "id": 1,
    "country": {
      "name": "Germany",
      "cities": [
        {
          "name": "Berlin",
          "population": 3.7,
          "sightseeing": ["Brandenburg Gate", "Reichstag"]
        },
        {
          "name": "Munich",
          "population": 1.5,
          "sightseeing": ["Marienplatz", "Olympiapark"]
        }
      ]
    }
  },
  {
    "id": 2,
    "country": {
      "name": "Japan",
      "cities": [
        {
          "name": "Tokyo",
          "population": 9.3,
          "sightseeing": ["Tokyo Tower", "Tokyo Skytree"]
        },
        {
          "name": "Osaka",
          "population": 2.7,
          "sightseeing": ["Osaka Castle", "Universal Studios Japan"]
        }
      ]
    }
  }
]

You can search on a nested field using a dot notation.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "should": [
            {
                "key": "country.name",
                "match": {
                    "value": "Germany"
                }
            }
        ]
    }
}

You can also search through arrays by projecting inner values using the [] syntax.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "should": [
            {
                "key": "country.cities[].population",
                "range": {
                    "gte": 9.0,
                }
            }
        ]
    }
}

This query would only output the point with id 2 as only Japan has a city with population greater than 9.0.

And the leaf nested field can also be an array.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "should": [
            {
                "key": "country.cities[].sightseeing",
                "match": {
                    "value": "Osaka Castle"
                }
            }
        ]
    }
}

This query would only output the point with id 2 as only Japan has a city with the “Osaka castke” as part of the sightseeing.

Nested object filter
Available as of v1.2.0

By default, the conditions are taking into account the entire payload of a point.

For instance, given two points with the following payload:

[
  {
    "id": 1,
    "dinosaur": "t-rex",
    "diet": [
      { "food": "leaves", "likes": false},
      { "food": "meat", "likes": true}
    ]
  },
  {
    "id": 2,
    "dinosaur": "diplodocus",
    "diet": [
      { "food": "leaves", "likes": true},
      { "food": "meat", "likes": false}
    ]
  }
]

The following query would match both points:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must": [
            {
                "key": "diet[].food",
                  "match": {
                    "value": "meat"
                }
            },
            {
                "key": "diet[].likes",
                  "match": {
                    "value": true
                }
            }
        ]
    }
}

This happens because both points are matching the two conditions:

the “t-rex” matches food=meat on diet[1].food and likes=true on diet[1].likes
the “diplodocus” matches food=meat on diet[1].food and likes=true on diet[0].likes
To retrieve only the points which are matching the conditions on an array element basis, that is the point with id 1 in this example, you would need to use a nested object filter.

Nested object filters allow arrays of objects to be queried independently of each other.

It is achieved by using the nested condition type formed by a payload key to focus on and a filter to apply.

The key should point to an array of objects and can be used with or without the bracket notation (“data” or “data[]”).

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must": [{
            "nested": {
                "key": "diet",
                "filter":{
                    "must": [
                        {
                            "key": "food",
                            "match": {
                                "value": "meat"
                            }
                        },
                        {
                            "key": "likes",
                            "match": {
                                "value": true
                            }
                        }
                    ]
                }
            }
        }]
    }
}

The matching logic is modified to be applied at the level of an array element within the payload.

Nested filters work in the same way as if the nested filter was applied to a single element of the array at a time. Parent document is considered to match the condition if at least one element of the array matches the nested filter.

Limitations

The has_id condition is not supported within the nested object filter. If you need it, place it in an adjacent must clause.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
   "filter":{
      "must":[
         {
            "nested":{
               "key":"diet",
               "filter":{
                  "must":[
                     {
                        "key":"food",
                        "match":{
                           "value":"meat"
                        }
                     },
                     {
                        "key":"likes",
                        "match":{
                           "value":true
                        }
                     }
                  ]
               }
            }
         },
         {
            "has_id":[
               1
            ]
         }
      ]
   }
}

Full Text Match
Available as of v0.10.0

A special case of the match condition is the text match condition. It allows you to search for a specific substring, token or phrase within the text field.

Exact texts that will match the condition depend on full-text index configuration. Configuration is defined during the index creation and describe at full-text index.

If there is no full-text index for the field, the condition will work as exact substring match.

json
python
typescript
rust
java
csharp
go
{
  "key": "description",
  "match": {
    "text": "good cheap"
  }
}

If the query has several words, then the condition will be satisfied only if all of them are present in the text.

Full Text Any
Available as of v1.16.0

The text_any full-text match condition is similar to the text condition, but with a key difference: while text only matches text fields that contain all the query terms, text_any matches fields that contain any of the query terms. In other words, even if a text field contains just one of the query terms, it is considered a match.

For example, a query for good cheap matches cheap hardware as well as good performance.

json
python
typescript
rust
java
csharp
go
{
  "key": "description",
  "match": {
    "text_any": "good cheap"
  }
}

Phrase Match
Available as of v1.15.0

A match phrase condition also leverages full-text index, to perform exact phrase comparisons. It allows you to search for a specific token phrase within the text field.

For example, the text "quick brown fox" will be matched by the query "brown fox", but not by "fox brown".

The index must be configured with phrase_matching parameter set to true. If the index has phrase matching disabled, phrase conditions won't match anything.
If there is no full-text index for the field, the condition will work as exact substring match.

json
python
typescript
rust
java
csharp
go
{
  "key": "description",
  "match": {
    "phrase": "brown fox"
  }
}

Range
json
python
typescript
rust
java
csharp
go
{
  "key": "price",
  "range": {
    "gt": null,
    "gte": 100.0,
    "lt": null,
    "lte": 450.0
  }
}

The range condition sets the range of possible values for stored payload values. If several values are stored, at least one of them should match the condition.

Comparisons that can be used:

gt - greater than
gte - greater than or equal
lt - less than
lte - less than or equal
Can be applied to float and integer payloads.

Datetime Range
The datetime range is a unique range condition, used for datetime payloads, which supports RFC 3339 formats. You do not need to convert dates to UNIX timestaps. During comparison, timestamps are parsed and converted to UTC.

Available as of v1.8.0

json
python
typescript
rust
java
csharp
go
{
  "key": "date",
  "range": {
    "gt": "2023-02-08T10:49:00Z",
    "gte": null,
    "lt": null,
    "lte": "2024-01-31 10:14:31Z"
  }
}

UUID Match
Available as of v1.11.0

Matching of UUID values works similarly to the regular match condition for strings. Functionally, it will work with keyword and uuid indexes exactly the same, but uuid index is more memory efficient.

json
python
typescript
rust
java
csharp
go
{
  "key": "uuid",
  "match": {
    "value": "f47ac10b-58cc-4372-a567-0e02b2c3d479"
  }
}

Geo
Geo Bounding Box
json
python
typescript
rust
java
csharp
go
{
  "key": "location",
  "geo_bounding_box": {
    "bottom_right": {
      "lon": 13.455868,
      "lat": 52.495862
    },
    "top_left": {
      "lon": 13.403683,
      "lat": 52.520711
    }
  }
}

It matches with locations inside a rectangle with the coordinates of the upper left corner in top_left and the coordinates of the lower right corner in bottom_right.

Geo Radius
json
python
typescript
rust
java
csharp
go
{
  "key": "location",
  "geo_radius": {
    "center": {
      "lon": 13.403683,
      "lat": 52.520711
    },
    "radius": 1000.0
  }
}

It matches with locations inside a circle with the center at the center and a radius of radius meters.

If several values are stored, at least one of them should match the condition. These conditions can only be applied to payloads that match the geo-data format.

Geo Polygon
Geo Polygons search is useful for when you want to find points inside an irregularly shaped area, for example a country boundary or a forest boundary. A polygon always has an exterior ring and may optionally include interior rings. A lake with an island would be an example of an interior ring. If you wanted to find points in the water but not on the island, you would make an interior ring for the island.

When defining a ring, you must pick either a clockwise or counterclockwise ordering for your points. The first and last point of the polygon must be the same.

Currently, we only support unprojected global coordinates (decimal degrees longitude and latitude) and we are datum agnostic.

json
python
typescript
rust
java
csharp
go

{
  "key": "location",
  "geo_polygon": {
    "exterior": {
      "points": [
        { "lon": -70.0, "lat": -70.0 },
        { "lon": 60.0, "lat": -70.0 },
        { "lon": 60.0, "lat": 60.0 },
        { "lon": -70.0, "lat": 60.0 },
        { "lon": -70.0, "lat": -70.0 }
      ]
    },
    "interiors": [
      {
        "points": [
          { "lon": -65.0, "lat": -65.0 },
          { "lon": 0.0, "lat": -65.0 },
          { "lon": 0.0, "lat": 0.0 },
          { "lon": -65.0, "lat": 0.0 },
          { "lon": -65.0, "lat": -65.0 }
        ]
      }
    ]
  }
}

A match is considered any point location inside or on the boundaries of the given polygon’s exterior but not inside any interiors.

If several location values are stored for a point, then any of them matching will include that point as a candidate in the resultset. These conditions can only be applied to payloads that match the geo-data format.

Values count
In addition to the direct value comparison, it is also possible to filter by the amount of values.

For example, given the data:

[
  { "id": 1, "name": "product A", "comments": ["Very good!", "Excellent"] },
  { "id": 2, "name": "product B", "comments": ["meh", "expected more", "ok"] }
]

We can perform the search only among the items with more than two comments:

json
python
typescript
rust
java
csharp
go
{
  "key": "comments",
  "values_count": {
    "gt": 2
  }
}

The result would be:

[{ "id": 2, "name": "product B", "comments": ["meh", "expected more", "ok"] }]

If stored value is not an array - it is assumed that the amount of values is equals to 1.

Is Empty
Sometimes it is also useful to filter out records that are missing some value. The IsEmpty condition may help you with that:

json
python
typescript
rust
java
csharp
go
{
  "is_empty": {
    "key": "reports"
  }
}

This condition will match all records where the field reports either does not exist, or has null or [] value.

The IsEmpty is often useful together with the logical negation must_not. In this case all non-empty values will be selected.
Is Null
It is not possible to test for NULL values with the match condition. We have to use IsNull condition instead:

json
python
typescript
rust
java
csharp
go
{
    "is_null": {
        "key": "reports"
    }
}

This condition will match all records where the field reports exists and has NULL value.

Has id
This type of query is not related to payload, but can be very useful in some situations. For example, the user could mark some specific search results as irrelevant, or we want to search only among the specified points.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must": [
            { "has_id": [1,3,5,7,9,11] }
        ]
    }
    ...
}

Filtered points would be:

[
  { "id": 1, "city": "London", "color": "green" },
  { "id": 3, "city": "London", "color": "blue" },
  { "id": 5, "city": "Moscow", "color": "green" }
]

Has vector
Available as of v1.13.0

This condition enables filtering by the presence of a given named vector on a point.

For example, if we have two named vector in our collection.

PUT /collections/{collection_name}
{
    "vectors": {
        "image": {
            "size": 4,
            "distance": "Dot"
        },
        "text": {
            "size": 8,
            "distance": "Cosine"
        }
    },
    "sparse_vectors": {
        "sparse-image": {},
        "sparse-text": {},
    },
}

Some points in the collection might have all vectors, some might have only a subset of them.

If your collection does not have named vectors, use an empty ("") name.
This is how you can search for points which have the dense image vector defined:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/scroll
{
    "filter": {
        "must": [
            { "has_vector": "image" }
        ]
    }
}