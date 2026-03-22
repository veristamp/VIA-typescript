Explore the data
After mastering the concepts in search, you can start exploring your data in other ways. Qdrant provides a stack of APIs that allow you to find similar vectors in a different fashion, as well as to find the most dissimilar ones. These are useful tools for recommendation systems, data exploration, and data cleaning.

Recommendation API
In addition to the regular search, Qdrant also allows you to search based on multiple positive and negative examples. The API is called recommend, and the examples can be point IDs, so that you can leverage the already encoded objects; and, as of v1.6, you can also use raw vectors as input, so that you can create your vectors on the fly without uploading them as points.

REST API - API Schema definition is available here

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/query
{
  "query": {
    "recommend": {
      "positive": [100, 231],
      "negative": [718, [0.2, 0.3, 0.4, 0.5]],
      "strategy": "average_vector"
    }
  },
  "filter": {
    "must": [
      {
        "key": "city",
        "match": {
          "value": "London"
        }
      }
    ]
  }
}

Example result of this API would be

{
  "result": [
    { "id": 10, "score": 0.81 },
    { "id": 14, "score": 0.75 },
    { "id": 11, "score": 0.73 }
  ],
  "status": "ok",
  "time": 0.001
}

The algorithm used to get the recommendations is selected from the available strategy options. Each of them has its own strengths and weaknesses, so experiment and choose the one that works best for your case.

Average vector strategy
The default and first strategy added to Qdrant is called average_vector. It preprocesses the input examples to create a single vector that is used for the search. Since the preprocessing step happens very fast, the performance of this strategy is on-par with regular search. The intuition behind this kind of recommendation is that each vector component represents an independent feature of the data, so, by averaging the examples, we should get a good recommendation.

The way to produce the searching vector is by first averaging all the positive and negative examples separately, and then combining them into a single vector using the following formula:

avg_positive + avg_positive - avg_negative

In the case of not having any negative examples, the search vector will simply be equal to avg_positive.

This is the default strategy that’s going to be set implicitly, but you can explicitly define it by setting "strategy": "average_vector" in the recommendation request.

Best score strategy
Available as of v1.6.0

A new strategy introduced in v1.6, is called best_score. It is based on the idea that the best way to find similar vectors is to find the ones that are closer to a positive example, while avoiding the ones that are closer to a negative one. The way it works is that each candidate is measured against every example, then we select the best positive and best negative scores. The final score is chosen with this step formula:

// Sigmoid function to normalize the score between 0 and 1
let sigmoid = |x| 0.5 * (1.0 + (x / (1.0 + x.abs())));

let score = if best_positive_score > best_negative_score {
    sigmoid(best_positive_score)
} else {
    -sigmoid(best_negative_score)
};

The performance of best_score strategy will be linearly impacted by the amount of examples.
Since we are computing similarities to every example at each step of the search, the performance of this strategy will be linearly impacted by the amount of examples. This means that the more examples you provide, the slower the search will be. However, this strategy can be very powerful and should be more embedding-agnostic.

Accuracy may be impacted with this strategy. To improve it, increasing the ef search parameter to something above 32 will already be much better than the default 16, e.g: "params": { "ef": 64 }
To use this algorithm, you need to set "strategy": "best_score" in the recommendation request.

Using only negative examples
A beneficial side-effect of best_score strategy is that you can use it with only negative examples. This will allow you to find the most dissimilar vectors to the ones you provide. This can be useful for finding outliers in your data, or for finding the most dissimilar vectors to a given one.

Combining negative-only examples with filtering can be a powerful tool for data exploration and cleaning.

Sum scores strategy
Another strategy for using multiple query vectors simultaneously is to just sum their scores against the candidates. In qdrant, this is called sum_scores strategy.

This strategy was used in this paper by UKP Lab, hessian.ai and cohere.ai to incorporate relevance feedback into a subsequent search. In the paper this boosted the nDCG@20 performance by 5.6% points when using 2-8 positive feedback documents.

The formula that this strategy implements is

  
 

where 
 is the set of positive examples, 
 is the set of negative examples, and 
 is the score of the vector 
 against the vector 

As with best_score, this strategy also allows using only negative examples.

Multiple vectors
Available as of v0.10.0

If the collection was created with multiple vectors, the name of the vector should be specified in the recommendation request:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/query
{
  "query": {
    "recommend": {
      "positive": [100, 231],
      "negative": [718]
    }
  },
  "using": "image",
  "limit": 10
}

Parameter using specifies which stored vectors to use for the recommendation.

Lookup vectors from another collection
Available as of v0.11.6

If you have collections with vectors of the same dimensionality, and you want to look for recommendations in one collection based on the vectors of another collection, you can use the lookup_from parameter.

It might be useful, e.g. in the item-to-user recommendations scenario. Where user and item embeddings, although having the same vector parameters (distance type and dimensionality), are usually stored in different collections.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/query
{
  "query": {
    "recommend": {
      "positive": [100, 231],
      "negative": [718]
    }
  },
  "limit": 10,
  "lookup_from": {
    "collection": "{external_collection_name}",
    "vector": "{external_vector_name}"
  }
}

Vectors are retrieved from the external collection by ids provided in the positive and negative lists. These vectors then used to perform the recommendation in the current collection, comparing against the “using” or default vector.

Batch recommendation API
Available as of v0.10.0

Similar to the batch search API in terms of usage and advantages, it enables the batching of recommendation requests.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/query/batch
{
  "searches": [
    {
      "query": {
        "recommend": {
          "positive": [100, 231],
          "negative": [718]
        }
      },
      "filter": {
        "must": [
          {
            "key": "city",
            "match": {
              "value": "London"
            }
          }
        ]
      },
      "limit": 10
    },
    {
      "query": {
        "recommend": {
          "positive": [200, 67],
          "negative": [300]
        }
      },
      "filter": {
        "must": [
          {
            "key": "city",
            "match": {
              "value": "London"
            }
          }
        ]
      },
      "limit": 10
    }
  ]
}

The result of this API contains one array per recommendation requests.

{
  "result": [
    [
        { "id": 10, "score": 0.81 },
        { "id": 14, "score": 0.75 },
        { "id": 11, "score": 0.73 }
    ],
    [
        { "id": 1, "score": 0.92 },
        { "id": 3, "score": 0.89 },
        { "id": 9, "score": 0.75 }
    ]
  ],
  "status": "ok",
  "time": 0.001
}

Discovery API
Available as of v1.7

REST API Schema definition available here

In this API, Qdrant introduces the concept of context, which is used for splitting the space. Context is a set of positive-negative pairs, and each pair divides the space into positive and negative zones. In that mode, the search operation prefers points based on how many positive zones they belong to (or how much they avoid negative zones).

The interface for providing context is similar to the recommendation API (ids or raw vectors). Still, in this case, they need to be provided in the form of positive-negative pairs.

Discovery API lets you do two new types of search:

Discovery search: Uses the context (the pairs of positive-negative vectors) and a target to return the points more similar to the target, but constrained by the context.
Context search: Using only the context pairs, get the points that live in the best zone, where loss is minimized
The way positive and negative examples should be arranged in the context pairs is completely up to you. So you can have the flexibility of trying out different permutation techniques based on your model and data.

The speed of search is linearly related to the amount of examples you provide in the query.
Discovery search
This type of search works specially well for combining multimodal, vector-constrained searches. Qdrant already has extensive support for filters, which constrain the search based on its payload, but using discovery search, you can also constrain the vector space in which the search is performed.

Discovery search

The formula for the discovery score can be expressed as:

 
where 
 represents a positive example, 
 represents a negative example, and 
 is the similarity score of a vector 
 to the target vector. The discovery score is then computed as:
where 
 is the similarity function, 
 is the target vector, and again 
 and 
 are the positive and negative examples, respectively. The sigmoid function is used to normalize the score between 0 and 1 and the sum of ranks is used to penalize vectors that are closer to the negative examples than to the positive ones. In other words, the sum of individual ranks determines how many positive zones a point is in, while the closeness hierarchy comes second.

Example:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/query
{
  "query": {
    "discover": {
      "target": [0.2, 0.1, 0.9, 0.7],
      "context": [
        {
          "positive": 100,
          "negative": 718
        },
        {
          "positive": 200,
          "negative": 300
        }
      ]
    }
  },
  "limit": 10
}

Notes about discovery search:
When providing ids as examples, they will be excluded from the results.
Score is always in descending order (larger is better), regardless of the metric used.
Since the space is hard-constrained by the context, accuracy is normal to drop when using default settings. To mitigate this, increasing the ef search parameter to something above 64 will already be much better than the default 16, e.g: "params": { "ef": 128 }
Context search
Conversely, in the absence of a target, a rigid integer-by-integer function doesn’t provide much guidance for the search when utilizing a proximity graph like HNSW. Instead, context search employs a function derived from the triplet-loss concept, which is usually applied during model training. For context search, this function is adapted to steer the search towards areas with fewer negative examples.

Context search

We can directly associate the score function to a loss function, where 0.0 is the maximum score a point can have, which means it is only in positive areas. As soon as a point exists closer to a negative example, its loss will simply be the difference of the positive and negative similarities.


Where 
 and 
 are the positive and negative examples of each pair, and 
 is the similarity function.

Using this kind of search, you can expect the output to not necessarily be around a single point, but rather, to be any point that isn’t closer to a negative example, which creates a constrained diverse result. So, even when the API is not called recommend, recommendation systems can also use this approach and adapt it for their specific use-cases.

Example:

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/query
{
  "query": {
    "context": [
      {
        "positive": 100,
        "negative": 718
      },
      {
        "positive": 200,
        "negative": 300
      }
    ]
  },
  "limit": 10
}

Notes about context search:
When providing ids as examples, they will be excluded from the results.
Score is always in descending order (larger is better), regardless of the metric used.
Best possible score is 0.0, and it is normal that many points get this score.
Distance Matrix
Available as of v1.12.0

The distance matrix API allows to calculate the distance between sampled pairs of vectors and to return the result as a sparse matrix.

Such API enables new data exploration use cases such as clustering similar vectors, visualization of connections or dimension reduction.

The API input request consists of the following parameters:

sample: the number of vectors to sample
limit: the number of scores to return per sample
filter: the filter to apply to constraint the samples
Let’s have a look at a basic example with sample=100, limit=10:

The engine starts by selecting 100 random points from the collection, then for each of the selected points, it will compute the top 10 closest points within the samples.

This will results in a total of 1000 scores represented as a sparse matrix for efficient processing.

The distance matrix API offers two output formats to ease the integration with different tools.

Pairwise format
Returns the distance matrix as a list of pairs of point ids with their respective score.

http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/search/matrix/pairs
{
    "sample": 10,
    "limit": 2,
    "filter": {
        "must": {
            "key": "color",
            "match": { "value": "red" }
        }
    }  
}

Returns

{
    "result": {
        "pairs": [
            {"a": 1, "b": 3, "score": 1.4063001},
            {"a": 1, "b": 4, "score": 1.2531},
            {"a": 2, "b": 1, "score": 1.1550001},
            {"a": 2, "b": 8, "score": 1.1359},
            {"a": 3, "b": 1, "score": 1.4063001},
            {"a": 3, "b": 4, "score": 1.2218001},
            {"a": 4, "b": 1, "score": 1.2531},
            {"a": 4, "b": 3, "score": 1.2218001},
            {"a": 5, "b": 3, "score": 0.70239997},
            {"a": 5, "b": 1, "score": 0.6146},
            {"a": 6, "b": 3, "score": 0.6353},
            {"a": 6, "b": 4, "score": 0.5093},
            {"a": 7, "b": 3, "score": 1.0990001},
            {"a": 7, "b": 1, "score": 1.0349001},
            {"a": 8, "b": 2, "score": 1.1359},
            {"a": 8, "b": 3, "score": 1.0553}
        ]
    }
}

Offset format
Returns the distance matrix as a four arrays:

offsets_row and offsets_col, represent the positions of non-zero distance values in the matrix.
scores contains the distance values.
ids contains the point ids corresponding to the distance values.
http
python
typescript
rust
java
csharp
go
POST /collections/{collection_name}/points/search/matrix/offsets
{
    "sample": 10,
    "limit": 2,
    "filter": {
        "must": {
            "key": "color",
            "match": { "value": "red" }
        }
    }
}

Returns

{
    "result": {
        "offsets_row": [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7],
        "offsets_col": [2, 3, 0, 7, 0, 3, 0, 2, 2, 0, 2, 3, 2, 0, 1, 2],
        "scores": [
            1.4063001, 1.2531, 1.1550001, 1.1359, 1.4063001,
            1.2218001, 1.2531, 1.2218001, 0.70239997, 0.6146, 0.6353,
            0.5093, 1.0990001, 1.0349001, 1.1359, 1.0553
            ],
        "ids": [1, 2, 3, 4, 5, 6, 7, 8]
    }
}