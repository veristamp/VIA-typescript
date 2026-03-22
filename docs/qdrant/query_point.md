# Query point groups

POST http://localhost:6333/collections/{collection_name}/points/query/groups
Content-Type: application/json

Universally query points and group results by a specified payload field. This endpoint covers all capabilities of search, recommend, discover, filters. But also enables hybrid and multi-stage queries.

Reference: https://api.qdrant.tech/api-reference/search/query-points-groups

## OpenAPI Specification

```yaml
openapi: 3.1.0
info:
  title: API
  version: 1.0.0
paths:
  /collections/{collection_name}/points/query/groups:
    post:
      operationId: query-points-groups
      summary: Query point groups
      description: >-
        Universally query points and group results by a specified payload field.
        This endpoint covers all capabilities of search, recommend, discover,
        filters. But also enables hybrid and multi-stage queries.
      tags:
        - subpackage_search
      parameters:
        - name: collection_name
          in: path
          description: Name of the collection to query
          required: true
          schema:
            type: string
        - name: consistency
          in: query
          description: Define read consistency guarantees for the operation
          required: false
          schema:
            $ref: '#/components/schemas/ReadConsistency'
        - name: timeout
          in: query
          description: If set, overrides global timeout for this request. Unit is seconds.
          required: false
          schema:
            type: integer
        - name: api-key
          in: header
          required: true
          schema:
            type: string
      responses:
        '200':
          description: successful operation
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Search_query_points_groups_Response_200'
      requestBody:
        description: Describes the query to make to the collection
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/QueryGroupsRequest'
servers:
  - url: http://localhost:6333
  - url: https://localhost:6333
components:
  schemas:
    ReadConsistencyType:
      type: string
      enum:
        - majority
        - quorum
        - all
      description: >-
        * `majority` - send N/2+1 random request and return points, which
        present on all of them


        * `quorum` - send requests to all nodes and return points which present
        on majority of nodes


        * `all` - send requests to all nodes and return points which present on
        all nodes
      title: ReadConsistencyType
    ReadConsistency:
      oneOf:
        - type: integer
        - $ref: '#/components/schemas/ReadConsistencyType'
      description: >-
        Read consistency parameter


        Defines how many replicas should be queried to get the result


        * `N` - send N random request and return points, which present on all of
        them


        * `majority` - send N/2+1 random request and return points, which
        present on all of them


        * `quorum` - send requests to all nodes and return points which present
        on majority of them


        * `all` - send requests to all nodes and return points which present on
        all of them


        Default value is `Factor(1)`
      title: ReadConsistency
    ShardKey:
      oneOf:
        - type: string
        - type: integer
          format: uint64
      title: ShardKey
    ShardKeySelector1:
      type: array
      items:
        $ref: '#/components/schemas/ShardKey'
      title: ShardKeySelector1
    ShardKeyWithFallback:
      type: object
      properties:
        target:
          $ref: '#/components/schemas/ShardKey'
        fallback:
          $ref: '#/components/schemas/ShardKey'
      required:
        - target
        - fallback
      title: ShardKeyWithFallback
    ShardKeySelector:
      oneOf:
        - $ref: '#/components/schemas/ShardKey'
        - $ref: '#/components/schemas/ShardKeySelector1'
        - $ref: '#/components/schemas/ShardKeyWithFallback'
      title: ShardKeySelector
    QueryGroupsRequestShardKey:
      oneOf:
        - $ref: '#/components/schemas/ShardKeySelector'
        - description: Any type
      title: QueryGroupsRequestShardKey
    PrefetchPrefetch1:
      type: array
      items:
        $ref: '#/components/schemas/Prefetch'
      title: PrefetchPrefetch1
    PrefetchPrefetch:
      oneOf:
        - $ref: '#/components/schemas/Prefetch'
        - $ref: '#/components/schemas/PrefetchPrefetch1'
        - description: Any type
      description: >-
        Sub-requests to perform first. If present, the query will be performed
        on the results of the prefetches.
      title: PrefetchPrefetch
    SparseVector:
      type: object
      properties:
        indices:
          type: array
          items:
            type: integer
            format: uint
          description: Indices must be unique
        values:
          type: array
          items:
            type: number
            format: double
          description: Values and indices must be the same length
      required:
        - indices
        - values
      description: Sparse vector structure
      title: SparseVector
    ExtendedPointId:
      oneOf:
        - type: integer
          format: uint64
        - type: string
          format: uuid
      description: Type, used for specifying point ID in user interface
      title: ExtendedPointId
    TokenizerType:
      type: string
      enum:
        - prefix
        - whitespace
        - word
        - multilingual
      title: TokenizerType
    Language:
      type: string
      enum:
        - arabic
        - azerbaijani
        - basque
        - bengali
        - catalan
        - chinese
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hebrew
        - hinglish
        - hungarian
        - indonesian
        - italian
        - japanese
        - kazakh
        - nepali
        - norwegian
        - portuguese
        - romanian
        - russian
        - slovene
        - spanish
        - swedish
        - tajik
        - turkish
      title: Language
    StopwordsSet:
      type: object
      properties:
        languages:
          type:
            - array
            - 'null'
          items:
            $ref: '#/components/schemas/Language'
          description: >-
            Set of languages to use for stopwords. Multiple pre-defined lists of
            stopwords can be combined.
        custom:
          type:
            - array
            - 'null'
          items:
            type: string
          description: Custom stopwords set. Will be merged with the languages set.
      title: StopwordsSet
    StopwordsInterface:
      oneOf:
        - $ref: '#/components/schemas/Language'
        - $ref: '#/components/schemas/StopwordsSet'
      title: StopwordsInterface
    Bm25ConfigStopwords:
      oneOf:
        - $ref: '#/components/schemas/StopwordsInterface'
        - description: Any type
      description: >-
        Configuration of the stopwords filter. Supports list of pre-defined
        languages and custom stopwords. Default: initialized for specified
        `language` or English if not specified.
      title: Bm25ConfigStopwords
    Snowball:
      type: string
      enum:
        - snowball
      title: Snowball
    SnowballLanguage:
      type: string
      enum:
        - arabic
        - armenian
        - danish
        - dutch
        - english
        - finnish
        - french
        - german
        - greek
        - hungarian
        - italian
        - norwegian
        - portuguese
        - romanian
        - russian
        - spanish
        - swedish
        - tamil
        - turkish
      description: Languages supported by snowball stemmer.
      title: SnowballLanguage
    SnowballParams:
      type: object
      properties:
        type:
          $ref: '#/components/schemas/Snowball'
        language:
          $ref: '#/components/schemas/SnowballLanguage'
      required:
        - type
        - language
      title: SnowballParams
    StemmingAlgorithm:
      oneOf:
        - $ref: '#/components/schemas/SnowballParams'
      description: Different stemming algorithms with their configs.
      title: StemmingAlgorithm
    Bm25ConfigStemmer:
      oneOf:
        - $ref: '#/components/schemas/StemmingAlgorithm'
        - description: Any type
      description: >-
        Configuration of the stemmer. Processes tokens to their root form.
        Default: initialized Snowball stemmer for specified `language` or
        English if not specified.
      title: Bm25ConfigStemmer
    Bm25Config:
      type: object
      properties:
        k:
          type: number
          format: double
          default: 1.2
          description: >-
            Controls term frequency saturation. Higher values mean term
            frequency has more impact. Default is 1.2
        b:
          type: number
          format: double
          default: 0.75
          description: >-
            Controls document length normalization. Ranges from 0 (no
            normalization) to 1 (full normalization). Higher values mean longer
            documents have less impact. Default is 0.75.
        avg_len:
          type: number
          format: double
          default: 256
          description: Expected average document length in the collection. Default is 256.
        tokenizer:
          $ref: '#/components/schemas/TokenizerType'
        language:
          type:
            - string
            - 'null'
          description: >-
            Defines which language to use for text preprocessing. This parameter
            is used to construct default stopwords filter and stemmer. To
            disable language-specific processing, set this to `"language":
            "none"`. If not specified, English is assumed.
        lowercase:
          type:
            - boolean
            - 'null'
          description: Lowercase the text before tokenization. Default is `true`.
        ascii_folding:
          type:
            - boolean
            - 'null'
          description: >-
            If true, normalize tokens by folding accented characters to ASCII
            (e.g., "ação" -> "acao"). Default is `false`.
        stopwords:
          $ref: '#/components/schemas/Bm25ConfigStopwords'
          description: >-
            Configuration of the stopwords filter. Supports list of pre-defined
            languages and custom stopwords. Default: initialized for specified
            `language` or English if not specified.
        stemmer:
          $ref: '#/components/schemas/Bm25ConfigStemmer'
          description: >-
            Configuration of the stemmer. Processes tokens to their root form.
            Default: initialized Snowball stemmer for specified `language` or
            English if not specified.
        min_token_len:
          type:
            - integer
            - 'null'
          description: >-
            Minimum token length to keep. If token is shorter than this, it will
            be discarded. Default is `None`, which means no minimum length.
        max_token_len:
          type:
            - integer
            - 'null'
          description: >-
            Maximum token length to keep. If token is longer than this, it will
            be discarded. Default is `None`, which means no maximum length.
      description: Configuration of the local bm25 models.
      title: Bm25Config
    DocumentOptions:
      oneOf:
        - type: object
          additionalProperties:
            description: Any type
        - $ref: '#/components/schemas/Bm25Config'
      description: >-
        Option variants for text documents. Ether general-purpose options or
        BM25-specific options. BM25-specific will only take effect if the
        `qdrant/bm25` is specified as a model.
      title: DocumentOptions
    Document:
      type: object
      properties:
        text:
          type: string
          description: >-
            Text of the document. This field will be used as input for the
            embedding model.
        model:
          type: string
          description: >-
            Name of the model used to generate the vector. List of available
            models depends on a provider.
        options:
          $ref: '#/components/schemas/DocumentOptions'
          description: >-
            Additional options for the model, will be passed to the inference
            service as-is. See model cards for available options.
      required:
        - text
        - model
      description: >-
        WARN: Work-in-progress, unimplemented


        Text document for embedding. Requires inference infrastructure,
        unimplemented.
      title: Document
    Image:
      type: object
      properties:
        image:
          description: 'Image data: base64 encoded image or an URL'
        model:
          type: string
          description: >-
            Name of the model used to generate the vector. List of available
            models depends on a provider.
        options:
          type:
            - object
            - 'null'
          additionalProperties:
            description: Any type
          description: Parameters for the model Values of the parameters are model-specific
      required:
        - image
        - model
      description: >-
        WARN: Work-in-progress, unimplemented


        Image object for embedding. Requires inference infrastructure,
        unimplemented.
      title: Image
    InferenceObject:
      type: object
      properties:
        object:
          description: >-
            Arbitrary data, used as input for the embedding model. Used if the
            model requires more than one input or a custom input.
        model:
          type: string
          description: >-
            Name of the model used to generate the vector. List of available
            models depends on a provider.
        options:
          type:
            - object
            - 'null'
          additionalProperties:
            description: Any type
          description: Parameters for the model Values of the parameters are model-specific
      required:
        - object
        - model
      description: >-
        WARN: Work-in-progress, unimplemented


        Custom object for embedding. Requires inference infrastructure,
        unimplemented.
      title: InferenceObject
    VectorInput:
      oneOf:
        - type: array
          items:
            type: number
            format: double
        - $ref: '#/components/schemas/SparseVector'
        - type: array
          items:
            type: array
            items:
              type: number
              format: double
        - $ref: '#/components/schemas/ExtendedPointId'
        - $ref: '#/components/schemas/Document'
        - $ref: '#/components/schemas/Image'
        - $ref: '#/components/schemas/InferenceObject'
      title: VectorInput
    Mmr:
      type: object
      properties:
        diversity:
          type:
            - number
            - 'null'
          format: double
          description: >-
            Tunable parameter for the MMR algorithm. Determines the balance
            between diversity and relevance.


            A higher value favors diversity (dissimilarity to selected results),
            while a lower value favors relevance (similarity to the query
            vector).


            Must be in the range [0, 1]. Default value is 0.5.
        candidates_limit:
          type:
            - integer
            - 'null'
          description: |-
            The maximum number of candidates to consider for re-ranking.

            If not specified, the `limit` value is used.
      description: Maximal Marginal Relevance (MMR) algorithm for re-ranking the points.
      title: Mmr
    NearestQueryMmr:
      oneOf:
        - $ref: '#/components/schemas/Mmr'
        - description: Any type
      description: >-
        Perform MMR (Maximal Marginal Relevance) reranking after search, using
        the same vector in this query to calculate relevance.
      title: NearestQueryMmr
    NearestQuery:
      type: object
      properties:
        nearest:
          $ref: '#/components/schemas/VectorInput'
        mmr:
          $ref: '#/components/schemas/NearestQueryMmr'
          description: >-
            Perform MMR (Maximal Marginal Relevance) reranking after search,
            using the same vector in this query to calculate relevance.
      required:
        - nearest
      title: NearestQuery
    RecommendStrategy:
      type: string
      enum:
        - average_vector
        - best_score
        - sum_scores
      description: >-
        How to use positive and negative examples to find the results, default
        is `average_vector`:


        * `average_vector` - Average positive and negative vectors and create a
        single query with the formula `query = avg_pos + avg_pos - avg_neg`.
        Then performs normal search.


        * `best_score` - Uses custom search objective. Each candidate is
        compared against all examples, its score is then chosen from the
        `max(max_pos_score, max_neg_score)`. If the `max_neg_score` is chosen
        then it is squared and negated, otherwise it is just the
        `max_pos_score`.


        * `sum_scores` - Uses custom search objective. Compares against all
        inputs, sums all the scores. Scores against positive vectors are added,
        against negatives are subtracted.
      title: RecommendStrategy
    RecommendInputStrategy:
      oneOf:
        - $ref: '#/components/schemas/RecommendStrategy'
        - description: Any type
      description: How to use the provided vectors to find the results
      title: RecommendInputStrategy
    RecommendInput:
      type: object
      properties:
        positive:
          type:
            - array
            - 'null'
          items:
            $ref: '#/components/schemas/VectorInput'
          description: Look for vectors closest to the vectors from these points
        negative:
          type:
            - array
            - 'null'
          items:
            $ref: '#/components/schemas/VectorInput'
          description: Try to avoid vectors like the vector from these points
        strategy:
          $ref: '#/components/schemas/RecommendInputStrategy'
          description: How to use the provided vectors to find the results
      title: RecommendInput
    RecommendQuery:
      type: object
      properties:
        recommend:
          $ref: '#/components/schemas/RecommendInput'
      required:
        - recommend
      title: RecommendQuery
    ContextPair:
      type: object
      properties:
        positive:
          $ref: '#/components/schemas/VectorInput'
        negative:
          $ref: '#/components/schemas/VectorInput'
      required:
        - positive
        - negative
      title: ContextPair
    DiscoverInputContext1:
      type: array
      items:
        $ref: '#/components/schemas/ContextPair'
      title: DiscoverInputContext1
    DiscoverInputContext:
      oneOf:
        - $ref: '#/components/schemas/ContextPair'
        - $ref: '#/components/schemas/DiscoverInputContext1'
        - description: Any type
      description: Search space will be constrained by these pairs of vectors
      title: DiscoverInputContext
    DiscoverInput:
      type: object
      properties:
        target:
          $ref: '#/components/schemas/VectorInput'
        context:
          $ref: '#/components/schemas/DiscoverInputContext'
          description: Search space will be constrained by these pairs of vectors
      required:
        - target
        - context
      title: DiscoverInput
    DiscoverQuery:
      type: object
      properties:
        discover:
          $ref: '#/components/schemas/DiscoverInput'
      required:
        - discover
      title: DiscoverQuery
    ContextInput1:
      type: array
      items:
        $ref: '#/components/schemas/ContextPair'
      title: ContextInput1
    ContextInput:
      oneOf:
        - $ref: '#/components/schemas/ContextPair'
        - $ref: '#/components/schemas/ContextInput1'
        - description: Any type
      title: ContextInput
    ContextQuery:
      type: object
      properties:
        context:
          $ref: '#/components/schemas/ContextInput'
      required:
        - context
      title: ContextQuery
    Direction:
      type: string
      enum:
        - asc
        - desc
      title: Direction
    OrderByDirection:
      oneOf:
        - $ref: '#/components/schemas/Direction'
        - description: Any type
      description: 'Direction of ordering: `asc` or `desc`. Default is ascending.'
      title: OrderByDirection
    StartFrom:
      oneOf:
        - type: integer
          format: int64
        - type: number
          format: double
        - type: string
          format: date-time
      title: StartFrom
    OrderByStartFrom:
      oneOf:
        - $ref: '#/components/schemas/StartFrom'
        - description: Any type
      description: >-
        Which payload value to start scrolling from. Default is the lowest value
        for `asc` and the highest for `desc`
      title: OrderByStartFrom
    OrderBy:
      type: object
      properties:
        key:
          type: string
          description: Payload key to order by
        direction:
          $ref: '#/components/schemas/OrderByDirection'
          description: 'Direction of ordering: `asc` or `desc`. Default is ascending.'
        start_from:
          $ref: '#/components/schemas/OrderByStartFrom'
          description: >-
            Which payload value to start scrolling from. Default is the lowest
            value for `asc` and the highest for `desc`
      required:
        - key
      title: OrderBy
    OrderByInterface:
      oneOf:
        - type: string
        - $ref: '#/components/schemas/OrderBy'
      title: OrderByInterface
    OrderByQuery:
      type: object
      properties:
        order_by:
          $ref: '#/components/schemas/OrderByInterface'
      required:
        - order_by
      title: OrderByQuery
    Fusion:
      type: string
      enum:
        - rrf
        - dbsf
      description: >-
        Fusion algorithm allows to combine results of multiple prefetches.


        Available fusion algorithms:


        * `rrf` - Reciprocal Rank Fusion (with default parameters) * `dbsf` -
        Distribution-Based Score Fusion
      title: Fusion
    FusionQuery:
      type: object
      properties:
        fusion:
          $ref: '#/components/schemas/Fusion'
      required:
        - fusion
      title: FusionQuery
    Rrf:
      type: object
      properties:
        k:
          type:
            - integer
            - 'null'
          description: K parameter for reciprocal rank fusion
        weights:
          type:
            - array
            - 'null'
          items:
            type: number
            format: double
          description: >-
            Weights for each prefetch source. Higher weight gives more influence
            on the final ranking. If not specified, all prefetches are weighted
            equally. The number of weights should match the number of
            prefetches.
      description: Parameters for Reciprocal Rank Fusion
      title: Rrf
    RrfQuery:
      type: object
      properties:
        rrf:
          $ref: '#/components/schemas/Rrf'
      required:
        - rrf
      title: RrfQuery
    ValueVariants:
      oneOf:
        - type: string
        - type: integer
          format: int64
        - type: boolean
      title: ValueVariants
    MatchValue:
      type: object
      properties:
        value:
          $ref: '#/components/schemas/ValueVariants'
      required:
        - value
      description: Exact match of the given value
      title: MatchValue
    MatchText:
      type: object
      properties:
        text:
          type: string
      required:
        - text
      description: Full-text match of the strings.
      title: MatchText
    MatchTextAny:
      type: object
      properties:
        text_any:
          type: string
      required:
        - text_any
      description: Full-text match of at least one token of the string.
      title: MatchTextAny
    MatchPhrase:
      type: object
      properties:
        phrase:
          type: string
      required:
        - phrase
      description: Full-text phrase match of the string.
      title: MatchPhrase
    AnyVariants:
      oneOf:
        - type: array
          items:
            type: string
        - type: array
          items:
            type: integer
            format: int64
      title: AnyVariants
    MatchAny:
      type: object
      properties:
        any:
          $ref: '#/components/schemas/AnyVariants'
      required:
        - any
      description: Exact match on any of the given values
      title: MatchAny
    MatchExcept:
      type: object
      properties:
        except:
          $ref: '#/components/schemas/AnyVariants'
      required:
        - except
      description: Should have at least one value not matching the any given values
      title: MatchExcept
    Match:
      oneOf:
        - $ref: '#/components/schemas/MatchValue'
        - $ref: '#/components/schemas/MatchText'
        - $ref: '#/components/schemas/MatchTextAny'
        - $ref: '#/components/schemas/MatchPhrase'
        - $ref: '#/components/schemas/MatchAny'
        - $ref: '#/components/schemas/MatchExcept'
      description: Match filter request
      title: Match
    FieldConditionMatch:
      oneOf:
        - $ref: '#/components/schemas/Match'
        - description: Any type
      description: Check if point has field with a given value
      title: FieldConditionMatch
    Range:
      type: object
      properties:
        lt:
          type:
            - number
            - 'null'
          format: double
          description: point.key < range.lt
        gt:
          type:
            - number
            - 'null'
          format: double
          description: point.key > range.gt
        gte:
          type:
            - number
            - 'null'
          format: double
          description: point.key >= range.gte
        lte:
          type:
            - number
            - 'null'
          format: double
          description: point.key <= range.lte
      description: Range filter request
      title: Range
    DatetimeRange:
      type: object
      properties:
        lt:
          type:
            - string
            - 'null'
          format: date-time
          description: point.key < range.lt
        gt:
          type:
            - string
            - 'null'
          format: date-time
          description: point.key > range.gt
        gte:
          type:
            - string
            - 'null'
          format: date-time
          description: point.key >= range.gte
        lte:
          type:
            - string
            - 'null'
          format: date-time
          description: point.key <= range.lte
      description: Range filter request
      title: DatetimeRange
    RangeInterface:
      oneOf:
        - $ref: '#/components/schemas/Range'
        - $ref: '#/components/schemas/DatetimeRange'
      title: RangeInterface
    FieldConditionRange:
      oneOf:
        - $ref: '#/components/schemas/RangeInterface'
        - description: Any type
      description: Check if points value lies in a given range
      title: FieldConditionRange
    GeoPoint:
      type: object
      properties:
        lon:
          type: number
          format: double
        lat:
          type: number
          format: double
      required:
        - lon
        - lat
      description: Geo point payload schema
      title: GeoPoint
    GeoBoundingBox:
      type: object
      properties:
        top_left:
          $ref: '#/components/schemas/GeoPoint'
        bottom_right:
          $ref: '#/components/schemas/GeoPoint'
      required:
        - top_left
        - bottom_right
      description: >-
        Geo filter request


        Matches coordinates inside the rectangle, described by coordinates of
        lop-left and bottom-right edges
      title: GeoBoundingBox
    FieldConditionGeoBoundingBox:
      oneOf:
        - $ref: '#/components/schemas/GeoBoundingBox'
        - description: Any type
      description: Check if points geolocation lies in a given area
      title: FieldConditionGeoBoundingBox
    GeoRadius:
      type: object
      properties:
        center:
          $ref: '#/components/schemas/GeoPoint'
        radius:
          type: number
          format: double
          description: Radius of the area in meters
      required:
        - center
        - radius
      description: >-
        Geo filter request


        Matches coordinates inside the circle of `radius` and center with
        coordinates `center`
      title: GeoRadius
    FieldConditionGeoRadius:
      oneOf:
        - $ref: '#/components/schemas/GeoRadius'
        - description: Any type
      description: Check if geo point is within a given radius
      title: FieldConditionGeoRadius
    GeoLineString:
      type: object
      properties:
        points:
          type: array
          items:
            $ref: '#/components/schemas/GeoPoint'
      required:
        - points
      description: Ordered sequence of GeoPoints representing the line
      title: GeoLineString
    GeoPolygon:
      type: object
      properties:
        exterior:
          $ref: '#/components/schemas/GeoLineString'
        interiors:
          type:
            - array
            - 'null'
          items:
            $ref: '#/components/schemas/GeoLineString'
          description: >-
            Interior lines (if present) bound holes within the surface each
            GeoLineString must consist of a minimum of 4 points, and the first
            and last points must be the same.
      required:
        - exterior
      description: >-
        Geo filter request


        Matches coordinates inside the polygon, defined by `exterior` and
        `interiors`
      title: GeoPolygon
    FieldConditionGeoPolygon:
      oneOf:
        - $ref: '#/components/schemas/GeoPolygon'
        - description: Any type
      description: Check if geo point is within a given polygon
      title: FieldConditionGeoPolygon
    ValuesCount:
      type: object
      properties:
        lt:
          type:
            - integer
            - 'null'
          description: point.key.length() < values_count.lt
        gt:
          type:
            - integer
            - 'null'
          description: point.key.length() > values_count.gt
        gte:
          type:
            - integer
            - 'null'
          description: point.key.length() >= values_count.gte
        lte:
          type:
            - integer
            - 'null'
          description: point.key.length() <= values_count.lte
      description: Values count filter request
      title: ValuesCount
    FieldConditionValuesCount:
      oneOf:
        - $ref: '#/components/schemas/ValuesCount'
        - description: Any type
      description: Check number of values of the field
      title: FieldConditionValuesCount
    FieldCondition:
      type: object
      properties:
        key:
          type: string
          description: Payload key
        match:
          $ref: '#/components/schemas/FieldConditionMatch'
          description: Check if point has field with a given value
        range:
          $ref: '#/components/schemas/FieldConditionRange'
          description: Check if points value lies in a given range
        geo_bounding_box:
          $ref: '#/components/schemas/FieldConditionGeoBoundingBox'
          description: Check if points geolocation lies in a given area
        geo_radius:
          $ref: '#/components/schemas/FieldConditionGeoRadius'
          description: Check if geo point is within a given radius
        geo_polygon:
          $ref: '#/components/schemas/FieldConditionGeoPolygon'
          description: Check if geo point is within a given polygon
        values_count:
          $ref: '#/components/schemas/FieldConditionValuesCount'
          description: Check number of values of the field
        is_empty:
          type:
            - boolean
            - 'null'
          description: >-
            Check that the field is empty, alternative syntax for `is_empty:
            "field_name"`
        is_null:
          type:
            - boolean
            - 'null'
          description: >-
            Check that the field is null, alternative syntax for `is_null:
            "field_name"`
      required:
        - key
      description: All possible payload filtering conditions
      title: FieldCondition
    PayloadField:
      type: object
      properties:
        key:
          type: string
          description: Payload field name
      required:
        - key
      description: Payload field
      title: PayloadField
    IsEmptyCondition:
      type: object
      properties:
        is_empty:
          $ref: '#/components/schemas/PayloadField'
      required:
        - is_empty
      description: Select points with empty payload for a specified field
      title: IsEmptyCondition
    IsNullCondition:
      type: object
      properties:
        is_null:
          $ref: '#/components/schemas/PayloadField'
      required:
        - is_null
      description: Select points with null payload for a specified field
      title: IsNullCondition
    HasIdCondition:
      type: object
      properties:
        has_id:
          type: array
          items:
            $ref: '#/components/schemas/ExtendedPointId'
      required:
        - has_id
      description: ID-based filtering condition
      title: HasIdCondition
    HasVectorCondition:
      type: object
      properties:
        has_vector:
          type: string
      required:
        - has_vector
      description: Filter points which have specific vector assigned
      title: HasVectorCondition
    FilterShould1:
      type: array
      items:
        $ref: '#/components/schemas/Condition'
      title: FilterShould1
    FilterShould:
      oneOf:
        - $ref: '#/components/schemas/Condition'
        - $ref: '#/components/schemas/FilterShould1'
        - description: Any type
      description: At least one of those conditions should match
      title: FilterShould
    MinShould:
      type: object
      properties:
        conditions:
          type: array
          items:
            $ref: '#/components/schemas/Condition'
        min_count:
          type: integer
      required:
        - conditions
        - min_count
      title: MinShould
    FilterMinShould:
      oneOf:
        - $ref: '#/components/schemas/MinShould'
        - description: Any type
      description: At least minimum amount of given conditions should match
      title: FilterMinShould
    FilterMust1:
      type: array
      items:
        $ref: '#/components/schemas/Condition'
      title: FilterMust1
    FilterMust:
      oneOf:
        - $ref: '#/components/schemas/Condition'
        - $ref: '#/components/schemas/FilterMust1'
        - description: Any type
      description: All conditions must match
      title: FilterMust
    FilterMustNot1:
      type: array
      items:
        $ref: '#/components/schemas/Condition'
      title: FilterMustNot1
    FilterMustNot:
      oneOf:
        - $ref: '#/components/schemas/Condition'
        - $ref: '#/components/schemas/FilterMustNot1'
        - description: Any type
      description: All conditions must NOT match
      title: FilterMustNot
    Filter:
      type: object
      properties:
        should:
          $ref: '#/components/schemas/FilterShould'
          description: At least one of those conditions should match
        min_should:
          $ref: '#/components/schemas/FilterMinShould'
          description: At least minimum amount of given conditions should match
        must:
          $ref: '#/components/schemas/FilterMust'
          description: All conditions must match
        must_not:
          $ref: '#/components/schemas/FilterMustNot'
          description: All conditions must NOT match
      title: Filter
    Nested:
      type: object
      properties:
        key:
          type: string
        filter:
          $ref: '#/components/schemas/Filter'
      required:
        - key
        - filter
      description: Select points with payload for a specified nested field
      title: Nested
    NestedCondition:
      type: object
      properties:
        nested:
          $ref: '#/components/schemas/Nested'
      required:
        - nested
      title: NestedCondition
    Condition:
      oneOf:
        - $ref: '#/components/schemas/FieldCondition'
        - $ref: '#/components/schemas/IsEmptyCondition'
        - $ref: '#/components/schemas/IsNullCondition'
        - $ref: '#/components/schemas/HasIdCondition'
        - $ref: '#/components/schemas/HasVectorCondition'
        - $ref: '#/components/schemas/NestedCondition'
        - $ref: '#/components/schemas/Filter'
      title: Condition
    GeoDistanceParams:
      type: object
      properties:
        origin:
          $ref: '#/components/schemas/GeoPoint'
        to:
          type: string
          description: Payload field with the destination geo point
      required:
        - origin
        - to
      title: GeoDistanceParams
    GeoDistance:
      type: object
      properties:
        geo_distance:
          $ref: '#/components/schemas/GeoDistanceParams'
      required:
        - geo_distance
      title: GeoDistance
    DatetimeExpression:
      type: object
      properties:
        datetime:
          type: string
      required:
        - datetime
      title: DatetimeExpression
    DatetimeKeyExpression:
      type: object
      properties:
        datetime_key:
          type: string
      required:
        - datetime_key
      title: DatetimeKeyExpression
    MultExpression:
      type: object
      properties:
        mult:
          type: array
          items:
            $ref: '#/components/schemas/Expression'
      required:
        - mult
      title: MultExpression
    SumExpression:
      type: object
      properties:
        sum:
          type: array
          items:
            $ref: '#/components/schemas/Expression'
      required:
        - sum
      title: SumExpression
    NegExpression:
      type: object
      properties:
        neg:
          $ref: '#/components/schemas/Expression'
      required:
        - neg
      title: NegExpression
    AbsExpression:
      type: object
      properties:
        abs:
          $ref: '#/components/schemas/Expression'
      required:
        - abs
      title: AbsExpression
    DivParams:
      type: object
      properties:
        left:
          $ref: '#/components/schemas/Expression'
        right:
          $ref: '#/components/schemas/Expression'
        by_zero_default:
          type:
            - number
            - 'null'
          format: double
      required:
        - left
        - right
      title: DivParams
    DivExpression:
      type: object
      properties:
        div:
          $ref: '#/components/schemas/DivParams'
      required:
        - div
      title: DivExpression
    SqrtExpression:
      type: object
      properties:
        sqrt:
          $ref: '#/components/schemas/Expression'
      required:
        - sqrt
      title: SqrtExpression
    PowParams:
      type: object
      properties:
        base:
          $ref: '#/components/schemas/Expression'
        exponent:
          $ref: '#/components/schemas/Expression'
      required:
        - base
        - exponent
      title: PowParams
    PowExpression:
      type: object
      properties:
        pow:
          $ref: '#/components/schemas/PowParams'
      required:
        - pow
      title: PowExpression
    ExpExpression:
      type: object
      properties:
        exp:
          $ref: '#/components/schemas/Expression'
      required:
        - exp
      title: ExpExpression
    Log10Expression:
      type: object
      properties:
        log10:
          $ref: '#/components/schemas/Expression'
      required:
        - log10
      title: Log10Expression
    LnExpression:
      type: object
      properties:
        ln:
          $ref: '#/components/schemas/Expression'
      required:
        - ln
      title: LnExpression
    DecayParamsExpressionTarget:
      oneOf:
        - $ref: '#/components/schemas/Expression'
        - description: Any type
      description: The target value to start decaying from. Defaults to 0.
      title: DecayParamsExpressionTarget
    DecayParamsExpression:
      type: object
      properties:
        x:
          $ref: '#/components/schemas/Expression'
        target:
          $ref: '#/components/schemas/DecayParamsExpressionTarget'
          description: The target value to start decaying from. Defaults to 0.
        scale:
          type:
            - number
            - 'null'
          format: double
          description: >-
            The scale factor of the decay, in terms of `x`. Defaults to 1.0.
            Must be a non-zero positive number.
        midpoint:
          type:
            - number
            - 'null'
          format: double
          description: >-
            The midpoint of the decay. Should be between 0 and 1.Defaults to
            0.5. Output will be this value when `|x - target| == scale`.
      required:
        - x
      title: DecayParamsExpression
    LinDecayExpression:
      type: object
      properties:
        lin_decay:
          $ref: '#/components/schemas/DecayParamsExpression'
      required:
        - lin_decay
      title: LinDecayExpression
    ExpDecayExpression:
      type: object
      properties:
        exp_decay:
          $ref: '#/components/schemas/DecayParamsExpression'
      required:
        - exp_decay
      title: ExpDecayExpression
    GaussDecayExpression:
      type: object
      properties:
        gauss_decay:
          $ref: '#/components/schemas/DecayParamsExpression'
      required:
        - gauss_decay
      title: GaussDecayExpression
    Expression:
      oneOf:
        - type: number
          format: double
        - type: string
        - $ref: '#/components/schemas/Condition'
        - $ref: '#/components/schemas/GeoDistance'
        - $ref: '#/components/schemas/DatetimeExpression'
        - $ref: '#/components/schemas/DatetimeKeyExpression'
        - $ref: '#/components/schemas/MultExpression'
        - $ref: '#/components/schemas/SumExpression'
        - $ref: '#/components/schemas/NegExpression'
        - $ref: '#/components/schemas/AbsExpression'
        - $ref: '#/components/schemas/DivExpression'
        - $ref: '#/components/schemas/SqrtExpression'
        - $ref: '#/components/schemas/PowExpression'
        - $ref: '#/components/schemas/ExpExpression'
        - $ref: '#/components/schemas/Log10Expression'
        - $ref: '#/components/schemas/LnExpression'
        - $ref: '#/components/schemas/LinDecayExpression'
        - $ref: '#/components/schemas/ExpDecayExpression'
        - $ref: '#/components/schemas/GaussDecayExpression'
      title: Expression
    FormulaQuery:
      type: object
      properties:
        formula:
          $ref: '#/components/schemas/Expression'
        defaults:
          type: object
          additionalProperties:
            description: Any type
      required:
        - formula
      title: FormulaQuery
    Sample:
      type: string
      enum:
        - random
      title: Sample
    SampleQuery:
      type: object
      properties:
        sample:
          $ref: '#/components/schemas/Sample'
      required:
        - sample
      title: SampleQuery
    FeedbackItem:
      type: object
      properties:
        example:
          $ref: '#/components/schemas/VectorInput'
        score:
          type: number
          format: double
      required:
        - example
        - score
      title: FeedbackItem
    NaiveFeedbackStrategyParams:
      type: object
      properties:
        a:
          type: number
          format: double
        b:
          type: number
          format: double
        c:
          type: number
          format: double
      required:
        - a
        - b
        - c
      title: NaiveFeedbackStrategyParams
    NaiveFeedbackStrategy:
      type: object
      properties:
        naive:
          $ref: '#/components/schemas/NaiveFeedbackStrategyParams'
      required:
        - naive
      title: NaiveFeedbackStrategy
    FeedbackStrategy:
      oneOf:
        - $ref: '#/components/schemas/NaiveFeedbackStrategy'
      title: FeedbackStrategy
    RelevanceFeedbackInput:
      type: object
      properties:
        target:
          $ref: '#/components/schemas/VectorInput'
        feedback:
          type: array
          items:
            $ref: '#/components/schemas/FeedbackItem'
        strategy:
          $ref: '#/components/schemas/FeedbackStrategy'
      required:
        - target
        - feedback
        - strategy
      title: RelevanceFeedbackInput
    RelevanceFeedbackQuery:
      type: object
      properties:
        relevance_feedback:
          $ref: '#/components/schemas/RelevanceFeedbackInput'
      required:
        - relevance_feedback
      title: RelevanceFeedbackQuery
    Query:
      oneOf:
        - $ref: '#/components/schemas/NearestQuery'
        - $ref: '#/components/schemas/RecommendQuery'
        - $ref: '#/components/schemas/DiscoverQuery'
        - $ref: '#/components/schemas/ContextQuery'
        - $ref: '#/components/schemas/OrderByQuery'
        - $ref: '#/components/schemas/FusionQuery'
        - $ref: '#/components/schemas/RrfQuery'
        - $ref: '#/components/schemas/FormulaQuery'
        - $ref: '#/components/schemas/SampleQuery'
        - $ref: '#/components/schemas/RelevanceFeedbackQuery'
      title: Query
    QueryInterface:
      oneOf:
        - $ref: '#/components/schemas/VectorInput'
        - $ref: '#/components/schemas/Query'
      title: QueryInterface
    PrefetchQuery:
      oneOf:
        - $ref: '#/components/schemas/QueryInterface'
        - description: Any type
      description: >-
        Query to perform. If missing without prefetches, returns points ordered
        by their IDs.
      title: PrefetchQuery
    PrefetchFilter:
      oneOf:
        - $ref: '#/components/schemas/Filter'
        - description: Any type
      description: >-
        Filter conditions - return only those points that satisfy the specified
        conditions.
      title: PrefetchFilter
    QuantizationSearchParams:
      type: object
      properties:
        ignore:
          type: boolean
          default: false
          description: If true, quantized vectors are ignored. Default is false.
        rescore:
          type:
            - boolean
            - 'null'
          description: >-
            If true, use original vectors to re-score top-k results. Might
            require more time in case if original vectors are stored on disk. If
            not set, qdrant decides automatically apply rescoring or not.
        oversampling:
          type:
            - number
            - 'null'
          format: double
          description: >-
            Oversampling factor for quantization. Default is 1.0.


            Defines how many extra vectors should be pre-selected using
            quantized index, and then re-scored using original vectors.


            For example, if `oversampling` is 2.4 and `limit` is 100, then 240
            vectors will be pre-selected using quantized index, and then top-100
            will be returned after re-scoring.
      description: Additional parameters of the search
      title: QuantizationSearchParams
    SearchParamsQuantization:
      oneOf:
        - $ref: '#/components/schemas/QuantizationSearchParams'
        - description: Any type
      description: Quantization params
      title: SearchParamsQuantization
    AcornSearchParams:
      type: object
      properties:
        enable:
          type: boolean
          default: false
          description: >-
            If true, then ACORN may be used for the HNSW search based on filters
            selectivity. Improves search recall for searches with multiple
            low-selectivity payload filters, at cost of performance.
        max_selectivity:
          type:
            - number
            - 'null'
          format: double
          description: >-
            Maximum selectivity of filters to enable ACORN.


            If estimated filters selectivity is higher than this value, ACORN
            will not be used. Selectivity is estimated as: `estimated number of
            points satisfying the filters / total number of points`.


            0.0 for never, 1.0 for always. Default is 0.4.
      description: ACORN-related search parameters
      title: AcornSearchParams
    SearchParamsAcorn:
      oneOf:
        - $ref: '#/components/schemas/AcornSearchParams'
        - description: Any type
      description: ACORN search params
      title: SearchParamsAcorn
    SearchParams:
      type: object
      properties:
        hnsw_ef:
          type:
            - integer
            - 'null'
          description: >-
            Params relevant to HNSW index Size of the beam in a beam-search.
            Larger the value - more accurate the result, more time required for
            search.
        exact:
          type: boolean
          default: false
          description: >-
            Search without approximation. If set to true, search may run long
            but with exact results.
        quantization:
          $ref: '#/components/schemas/SearchParamsQuantization'
          description: Quantization params
        indexed_only:
          type: boolean
          default: false
          description: >-
            If enabled, the engine will only perform search among indexed or
            small segments. Using this option prevents slow searches in case of
            delayed index, but does not guarantee that all uploaded vectors will
            be included in search results
        acorn:
          $ref: '#/components/schemas/SearchParamsAcorn'
          description: ACORN search params
      description: Additional parameters of the search
      title: SearchParams
    PrefetchParams:
      oneOf:
        - $ref: '#/components/schemas/SearchParams'
        - description: Any type
      description: Search params for when there is no prefetch
      title: PrefetchParams
    LookupLocationShardKey:
      oneOf:
        - $ref: '#/components/schemas/ShardKeySelector'
        - description: Any type
      description: >-
        Specify in which shards to look for the points, if not specified - look
        in all shards
      title: LookupLocationShardKey
    LookupLocation:
      type: object
      properties:
        collection:
          type: string
          description: Name of the collection used for lookup
        vector:
          type:
            - string
            - 'null'
          description: >-
            Optional name of the vector field within the collection. If not
            provided, the default vector field will be used.
        shard_key:
          $ref: '#/components/schemas/LookupLocationShardKey'
          description: >-
            Specify in which shards to look for the points, if not specified -
            look in all shards
      required:
        - collection
      description: >-
        Defines a location to use for looking up the vector. Specifies
        collection and vector field name.
      title: LookupLocation
    PrefetchLookupFrom:
      oneOf:
        - $ref: '#/components/schemas/LookupLocation'
        - description: Any type
      description: >-
        The location to use for IDs lookup, if not specified - use the current
        collection and the 'using' vector Note: the other collection vectors
        should have the same vector size as the 'using' vector in the current
        collection
      title: PrefetchLookupFrom
    Prefetch:
      type: object
      properties:
        prefetch:
          $ref: '#/components/schemas/PrefetchPrefetch'
          description: >-
            Sub-requests to perform first. If present, the query will be
            performed on the results of the prefetches.
        query:
          $ref: '#/components/schemas/PrefetchQuery'
          description: >-
            Query to perform. If missing without prefetches, returns points
            ordered by their IDs.
        using:
          type:
            - string
            - 'null'
          description: >-
            Define which vector name to use for querying. If missing, the
            default vector is used.
        filter:
          $ref: '#/components/schemas/PrefetchFilter'
          description: >-
            Filter conditions - return only those points that satisfy the
            specified conditions.
        params:
          $ref: '#/components/schemas/PrefetchParams'
          description: Search params for when there is no prefetch
        score_threshold:
          type:
            - number
            - 'null'
          format: double
          description: Return points with scores better than this threshold.
        limit:
          type:
            - integer
            - 'null'
          description: Max number of points to return. Default is 10.
        lookup_from:
          $ref: '#/components/schemas/PrefetchLookupFrom'
          description: >-
            The location to use for IDs lookup, if not specified - use the
            current collection and the 'using' vector Note: the other collection
            vectors should have the same vector size as the 'using' vector in
            the current collection
      title: Prefetch
    QueryGroupsRequestPrefetch1:
      type: array
      items:
        $ref: '#/components/schemas/Prefetch'
      title: QueryGroupsRequestPrefetch1
    QueryGroupsRequestPrefetch:
      oneOf:
        - $ref: '#/components/schemas/Prefetch'
        - $ref: '#/components/schemas/QueryGroupsRequestPrefetch1'
        - description: Any type
      description: >-
        Sub-requests to perform first. If present, the query will be performed
        on the results of the prefetch(es).
      title: QueryGroupsRequestPrefetch
    QueryGroupsRequestQuery:
      oneOf:
        - $ref: '#/components/schemas/QueryInterface'
        - description: Any type
      description: >-
        Query to perform. If missing without prefetches, returns points ordered
        by their IDs.
      title: QueryGroupsRequestQuery
    QueryGroupsRequestFilter:
      oneOf:
        - $ref: '#/components/schemas/Filter'
        - description: Any type
      description: >-
        Filter conditions - return only those points that satisfy the specified
        conditions.
      title: QueryGroupsRequestFilter
    QueryGroupsRequestParams:
      oneOf:
        - $ref: '#/components/schemas/SearchParams'
        - description: Any type
      description: Search params for when there is no prefetch
      title: QueryGroupsRequestParams
    WithVector:
      oneOf:
        - type: boolean
        - type: array
          items:
            type: string
      description: Options for specifying which vector to include
      title: WithVector
    QueryGroupsRequestWithVector:
      oneOf:
        - $ref: '#/components/schemas/WithVector'
        - description: Any type
      description: >-
        Options for specifying which vectors to include into the response.
        Default is false.
      title: QueryGroupsRequestWithVector
    PayloadSelectorInclude:
      type: object
      properties:
        include:
          type: array
          items:
            type: string
          description: Only include this payload keys
      required:
        - include
      title: PayloadSelectorInclude
    PayloadSelectorExclude:
      type: object
      properties:
        exclude:
          type: array
          items:
            type: string
          description: Exclude this fields from returning payload
      required:
        - exclude
      title: PayloadSelectorExclude
    PayloadSelector:
      oneOf:
        - $ref: '#/components/schemas/PayloadSelectorInclude'
        - $ref: '#/components/schemas/PayloadSelectorExclude'
      description: Specifies how to treat payload selector
      title: PayloadSelector
    WithPayloadInterface:
      oneOf:
        - type: boolean
        - type: array
          items:
            type: string
        - $ref: '#/components/schemas/PayloadSelector'
      description: Options for specifying which payload to include or not
      title: WithPayloadInterface
    QueryGroupsRequestWithPayload:
      oneOf:
        - $ref: '#/components/schemas/WithPayloadInterface'
        - description: Any type
      description: >-
        Options for specifying which payload to include or not. Default is
        false.
      title: QueryGroupsRequestWithPayload
    QueryGroupsRequestLookupFrom:
      oneOf:
        - $ref: '#/components/schemas/LookupLocation'
        - description: Any type
      description: >-
        The location to use for IDs lookup, if not specified - use the current
        collection and the 'using' vector Note: the other collection vectors
        should have the same vector size as the 'using' vector in the current
        collection
      title: QueryGroupsRequestLookupFrom
    WithLookupWithPayload:
      oneOf:
        - $ref: '#/components/schemas/WithPayloadInterface'
        - description: Any type
      description: Options for specifying which payload to include (or not)
      title: WithLookupWithPayload
    WithLookupWithVectors:
      oneOf:
        - $ref: '#/components/schemas/WithVector'
        - description: Any type
      description: Options for specifying which vectors to include (or not)
      title: WithLookupWithVectors
    WithLookup:
      type: object
      properties:
        collection:
          type: string
          description: Name of the collection to use for points lookup
        with_payload:
          $ref: '#/components/schemas/WithLookupWithPayload'
          description: Options for specifying which payload to include (or not)
        with_vectors:
          $ref: '#/components/schemas/WithLookupWithVectors'
          description: Options for specifying which vectors to include (or not)
      required:
        - collection
      title: WithLookup
    WithLookupInterface:
      oneOf:
        - type: string
        - $ref: '#/components/schemas/WithLookup'
      title: WithLookupInterface
    QueryGroupsRequestWithLookup:
      oneOf:
        - $ref: '#/components/schemas/WithLookupInterface'
        - description: Any type
      description: Look for points in another collection using the group ids
      title: QueryGroupsRequestWithLookup
    QueryGroupsRequest:
      type: object
      properties:
        shard_key:
          $ref: '#/components/schemas/QueryGroupsRequestShardKey'
        prefetch:
          $ref: '#/components/schemas/QueryGroupsRequestPrefetch'
          description: >-
            Sub-requests to perform first. If present, the query will be
            performed on the results of the prefetch(es).
        query:
          $ref: '#/components/schemas/QueryGroupsRequestQuery'
          description: >-
            Query to perform. If missing without prefetches, returns points
            ordered by their IDs.
        using:
          type:
            - string
            - 'null'
          description: >-
            Define which vector name to use for querying. If missing, the
            default vector is used.
        filter:
          $ref: '#/components/schemas/QueryGroupsRequestFilter'
          description: >-
            Filter conditions - return only those points that satisfy the
            specified conditions.
        params:
          $ref: '#/components/schemas/QueryGroupsRequestParams'
          description: Search params for when there is no prefetch
        score_threshold:
          type:
            - number
            - 'null'
          format: double
          description: Return points with scores better than this threshold.
        with_vector:
          $ref: '#/components/schemas/QueryGroupsRequestWithVector'
          description: >-
            Options for specifying which vectors to include into the response.
            Default is false.
        with_payload:
          $ref: '#/components/schemas/QueryGroupsRequestWithPayload'
          description: >-
            Options for specifying which payload to include or not. Default is
            false.
        lookup_from:
          $ref: '#/components/schemas/QueryGroupsRequestLookupFrom'
          description: >-
            The location to use for IDs lookup, if not specified - use the
            current collection and the 'using' vector Note: the other collection
            vectors should have the same vector size as the 'using' vector in
            the current collection
        group_by:
          type: string
          description: >-
            Payload field to group by, must be a string or number field. If the
            field contains more than 1 value, all values will be used for
            grouping. One point can be in multiple groups.
        group_size:
          type:
            - integer
            - 'null'
          description: Maximum amount of points to return per group. Default is 3.
        limit:
          type:
            - integer
            - 'null'
          description: Maximum amount of groups to return. Default is 10.
        with_lookup:
          $ref: '#/components/schemas/QueryGroupsRequestWithLookup'
          description: Look for points in another collection using the group ids
      required:
        - group_by
      title: QueryGroupsRequest
    HardwareUsage:
      type: object
      properties:
        cpu:
          type: integer
        payload_io_read:
          type: integer
        payload_io_write:
          type: integer
        payload_index_io_read:
          type: integer
        payload_index_io_write:
          type: integer
        vector_io_read:
          type: integer
        vector_io_write:
          type: integer
      required:
        - cpu
        - payload_io_read
        - payload_io_write
        - payload_index_io_read
        - payload_index_io_write
        - vector_io_read
        - vector_io_write
      description: Usage of the hardware resources, spent to process the request
      title: HardwareUsage
    UsageHardware:
      oneOf:
        - $ref: '#/components/schemas/HardwareUsage'
        - description: Any type
      title: UsageHardware
    ModelUsage:
      type: object
      properties:
        tokens:
          type: integer
          format: uint64
      required:
        - tokens
      title: ModelUsage
    InferenceUsage:
      type: object
      properties:
        models:
          type: object
          additionalProperties:
            $ref: '#/components/schemas/ModelUsage'
      required:
        - models
      title: InferenceUsage
    UsageInference:
      oneOf:
        - $ref: '#/components/schemas/InferenceUsage'
        - description: Any type
      title: UsageInference
    Usage:
      type: object
      properties:
        hardware:
          $ref: '#/components/schemas/UsageHardware'
        inference:
          $ref: '#/components/schemas/UsageInference'
      description: Usage of the hardware resources, spent to process the request
      title: Usage
    CollectionsCollectionNamePointsQueryGroupsPostResponsesContentApplicationJsonSchemaUsage:
      oneOf:
        - $ref: '#/components/schemas/Usage'
        - description: Any type
      title: >-
        CollectionsCollectionNamePointsQueryGroupsPostResponsesContentApplicationJsonSchemaUsage
    Payload:
      type: object
      additionalProperties:
        description: Any type
      title: Payload
    ScoredPointPayload:
      oneOf:
        - $ref: '#/components/schemas/Payload'
        - description: Any type
      description: Payload - values assigned to the point
      title: ScoredPointPayload
    VectorOutput:
      oneOf:
        - type: array
          items:
            type: number
            format: double
        - $ref: '#/components/schemas/SparseVector'
        - type: array
          items:
            type: array
            items:
              type: number
              format: double
      description: Vector Data stored in Point
      title: VectorOutput
    VectorStructOutput2:
      type: object
      additionalProperties:
        $ref: '#/components/schemas/VectorOutput'
      title: VectorStructOutput2
    VectorStructOutput:
      oneOf:
        - type: array
          items:
            type: number
            format: double
        - type: array
          items:
            type: array
            items:
              type: number
              format: double
        - $ref: '#/components/schemas/VectorStructOutput2'
      description: Vector data stored in Point
      title: VectorStructOutput
    ScoredPointVector:
      oneOf:
        - $ref: '#/components/schemas/VectorStructOutput'
        - description: Any type
      description: Vector of the point
      title: ScoredPointVector
    ScoredPointShardKey:
      oneOf:
        - $ref: '#/components/schemas/ShardKey'
        - description: Any type
      description: Shard Key
      title: ScoredPointShardKey
    OrderValue:
      oneOf:
        - type: integer
          format: int64
        - type: number
          format: double
      title: OrderValue
    ScoredPointOrderValue:
      oneOf:
        - $ref: '#/components/schemas/OrderValue'
        - description: Any type
      description: Order-by value
      title: ScoredPointOrderValue
    ScoredPoint:
      type: object
      properties:
        id:
          $ref: '#/components/schemas/ExtendedPointId'
        version:
          type: integer
          format: uint64
          description: Point version
        score:
          type: number
          format: double
          description: Points vector distance to the query vector
        payload:
          $ref: '#/components/schemas/ScoredPointPayload'
          description: Payload - values assigned to the point
        vector:
          $ref: '#/components/schemas/ScoredPointVector'
          description: Vector of the point
        shard_key:
          $ref: '#/components/schemas/ScoredPointShardKey'
          description: Shard Key
        order_value:
          $ref: '#/components/schemas/ScoredPointOrderValue'
          description: Order-by value
      required:
        - id
        - version
        - score
      description: Search result
      title: ScoredPoint
    GroupId:
      oneOf:
        - type: string
        - type: integer
          format: uint64
        - type: integer
          format: int64
      description: Value of the group_by key, shared across all the hits in the group
      title: GroupId
    RecordPayload:
      oneOf:
        - $ref: '#/components/schemas/Payload'
        - description: Any type
      description: Payload - values assigned to the point
      title: RecordPayload
    RecordVector:
      oneOf:
        - $ref: '#/components/schemas/VectorStructOutput'
        - description: Any type
      description: Vector of the point
      title: RecordVector
    RecordShardKey:
      oneOf:
        - $ref: '#/components/schemas/ShardKey'
        - description: Any type
      description: Shard Key
      title: RecordShardKey
    RecordOrderValue:
      oneOf:
        - $ref: '#/components/schemas/OrderValue'
        - description: Any type
      title: RecordOrderValue
    Record:
      type: object
      properties:
        id:
          $ref: '#/components/schemas/ExtendedPointId'
        payload:
          $ref: '#/components/schemas/RecordPayload'
          description: Payload - values assigned to the point
        vector:
          $ref: '#/components/schemas/RecordVector'
          description: Vector of the point
        shard_key:
          $ref: '#/components/schemas/RecordShardKey'
          description: Shard Key
        order_value:
          $ref: '#/components/schemas/RecordOrderValue'
      required:
        - id
      description: Point data
      title: Record
    PointGroupLookup:
      oneOf:
        - $ref: '#/components/schemas/Record'
        - description: Any type
      description: Record that has been looked up using the group id
      title: PointGroupLookup
    PointGroup:
      type: object
      properties:
        hits:
          type: array
          items:
            $ref: '#/components/schemas/ScoredPoint'
          description: Scored points that have the same value of the group_by key
        id:
          $ref: '#/components/schemas/GroupId'
        lookup:
          $ref: '#/components/schemas/PointGroupLookup'
          description: Record that has been looked up using the group id
      required:
        - hits
        - id
      title: PointGroup
    GroupsResult:
      type: object
      properties:
        groups:
          type: array
          items:
            $ref: '#/components/schemas/PointGroup'
      required:
        - groups
      title: GroupsResult
    Search_query_points_groups_Response_200:
      type: object
      properties:
        usage:
          $ref: >-
            #/components/schemas/CollectionsCollectionNamePointsQueryGroupsPostResponsesContentApplicationJsonSchemaUsage
        time:
          type: number
          format: double
          description: Time spent to process this request
        status:
          type: string
        result:
          $ref: '#/components/schemas/GroupsResult'
      title: Search_query_points_groups_Response_200
  securitySchemes:
    default:
      type: apiKey
      in: header
      name: api-key

```

## SDK Code Examples

```python
from qdrant_client import QdrantClient, models

client = QdrantClient(url="http://localhost:6333")

client.query_points_groups(
    collection_name="{collection_name}",
    query=[0.01, 0.45, 0.67],
    group_by="document_id",
    limit=10,
    group_size=5,
)
```

```rust
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{Query, QueryPointGroupsBuilder};

let client = Qdrant::from_url("http://localhost:6334").build()?;

client.query_groups(
    QueryPointGroupsBuilder::new("{collection_name}", "document_id")
        .query(Query::from(vec![0.01, 0.45, 0.67]))
        .limit(10u64)
        .group_size(5u64)
).await?;

```

```java
import java.util.List;

import static io.qdrant.client.QueryFactory.nearest;

import io.qdrant.client.grpc.Points.QueryPointGroups;
import io.qdrant.client.grpc.Points.QueryPoints;


QdrantClient client =
    new QdrantClient(QdrantGrpcClient.newBuilder("localhost", 6334, false).build());

client
    .queryGroupsAsync(
        QueryPointGroups.newBuilder()
            .setCollectionName("{collection_name}")
            .setGroupBy("document_id")
            .setGroupSize(5)
            .setLimit(10)
            .setQuery(nearest(List.of(0.01f, 0.45f, 0.67f)))
            .build())
    .get();

```

```typescript
import { QdrantClient } from "@qdrant/js-client-rest";

const client = new QdrantClient({ host: "localhost", port: 6333 });

client.queryGroups("{collection_name}", {
    query: [0.01, 0.45, 0.67],
    group_by: "document_id",
    limit: 10,
    group_size: 5,
});

```

```go
package client

import (
	"context"
	"fmt"

	"github.com/qdrant/go-client/qdrant"
)

func queryGroups() {
	client, err := qdrant.NewClient(&qdrant.Config{
		Host: "localhost",
		Port: 6334,
	})
	if err != nil {
		panic(err)
	}

	groupSize := uint64(5)
	results, err := client.QueryGroups(context.Background(), &qdrant.QueryPointGroups{
		CollectionName: "{collection_name}",
		Query:          qdrant.NewQuery(0.01, 0.45, 0.67),
		GroupBy:        "document_id",
		GroupSize:      &groupSize,
	})
	if err != nil {
		panic(err)
	}
	fmt.Println("Query results: ", results)
}

```

```csharp
using Qdrant.Client;

var client = new QdrantClient("localhost", 6334);

await client.QueryGroupsAsync(
  collectionName: "{collection_name}",
  groupBy: "document_id",
  groupSize: 5,
  limit: 10,
  query: new float[] {
    0.01f, 0.45f, 0.67f
  }
);

```

```ruby
require 'uri'
require 'net/http'

url = URI("http://localhost:6333/collections/collection_name/points/query/groups")

http = Net::HTTP.new(url.host, url.port)

request = Net::HTTP::Post.new(url)
request["api-key"] = '<apiKey>'
request["Content-Type"] = 'application/json'
request.body = "{\n  \"group_by\": \"string\"\n}"

response = http.request(request)
puts response.read_body
```

```php
<?php
require_once('vendor/autoload.php');

$client = new \GuzzleHttp\Client();

$response = $client->request('POST', 'http://localhost:6333/collections/collection_name/points/query/groups', [
  'body' => '{
  "group_by": "string"
}',
  'headers' => [
    'Content-Type' => 'application/json',
    'api-key' => '<apiKey>',
  ],
]);

echo $response->getBody();
```

```swift
import Foundation

let headers = [
  "api-key": "<apiKey>",
  "Content-Type": "application/json"
]
let parameters = ["group_by": "string"] as [String : Any]

let postData = JSONSerialization.data(withJSONObject: parameters, options: [])

let request = NSMutableURLRequest(url: NSURL(string: "http://localhost:6333/collections/collection_name/points/query/groups")! as URL,
                                        cachePolicy: .useProtocolCachePolicy,
                                    timeoutInterval: 10.0)
request.httpMethod = "POST"
request.allHTTPHeaderFields = headers
request.httpBody = postData as Data

let session = URLSession.shared
let dataTask = session.dataTask(with: request as URLRequest, completionHandler: { (data, response, error) -> Void in
  if (error != nil) {
    print(error as Any)
  } else {
    let httpResponse = response as? HTTPURLResponse
    print(httpResponse)
  }
})

dataTask.resume()
```