
{} (:package |calcit)
  :configs $ {} (:init-fn |calcit.core/println!) (:reload-fn |calcit.core/println!) (:version |0.0.0)
    :modules $ []
  :files $ {}
    |calcit.core $ %{} :FileEntry
      :defs $ {}
        |%<- $ %{} :CodeEntry (:doc "|pass value as `%` into several expressions, in reversed order")
          :code $ quote
            defmacro %<- (& xs)
              quasiquote $ ->%
                ~@ $ reverse xs
        |%{} $ %{} :CodeEntry (:doc "|Low-level helper for constructing records and attaching metadata\nWraps `&%{` so higher-level macros like `defrecord!` can register methods.")
          :code $ quote
            defmacro %{} (R & xs)
              &let
                args $ &list:concat & xs
                quasiquote $ &%{} ~R ~@args
          :examples $ []
            quote $ let
                rec $ %{}
                  new-record :point
                  [] :x 1
                  [] :y 2
              assert= :point $ &tuple:nth rec 0
            quote $ assert= (new-record :point (:x 1) (:y 2)) $ %{}
              new-record :point
              [] :x 1
              [] :y 2
        |&<= $ %{} :CodeEntry (:doc "|Less than or equal comparison for two values")
          :code $ quote
            defn &<= (a b)
              hint-fn $ return-type :bool
              assert-type a :number
              assert-type b :number
              assert "|expects numbers for &<=" $ if (number? a) (number? b)
              if (&< a b) true $ &= a b
          :examples $ []
            quote $ assert= true $ &<= 3 5
            quote $ assert= true $ &<= 5 5
            quote $ assert= false $ &<= 5 3
        |&>= $ %{} :CodeEntry (:doc "|Greater than or equal comparison for two values")
          :code $ quote
            defn &>= (a b)
              hint-fn $ return-type :bool
              assert-type a :number
              assert-type b :number
              assert "|expects numbers for &>=" $ if (number? a) (number? b)
              if (&> a b) true $ &= a b
          :examples $ []
            quote $ assert= true $ &>= 5 3
            quote $ assert= true $ &>= 5 5
            quote $ assert= false $ &>= 3 5
        |&case $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &case (item default pattern & others)
              if
                not $ and (list? pattern)
                  &= 2 $ &list:count pattern
                raise $ str-spaced "|`case` expects pattern in a pair, got:" pattern
              let
                  x $ &list:first pattern
                  branch $ last pattern
                quasiquote $ if (&= ~item ~x) ~branch
                  ~ $ if (&list:empty? others) default
                    quasiquote $ &case ~item ~default ~@others
        |&core-fn-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! &core-fn-methods
              :call $ defn &fn:call (f & args) (f & args)
              :call-args $ defn &fn:call-args (f args) (f & args)
              :map $ defn &fn:map (f g)
                defn &fn:map (x)
                  f $ g x
              :bind $ defn &fn:bind (m f)
                defn %&fn:bind (x)
                  f (m x) x
              :mappend $ defn &fn:mappend (f g)
                defn %&fn:mappend (x)
                  .mappend (f x) (g x)
              :apply $ defn &fn:apply (f g)
                defn %*fn:apply (x)
                  g x $ f x
        |with-impls $ %{} :CodeEntry (:doc "|Assign trait implementations by value kind\nSyntax: (with-impls value klasses)\nParams: value (record/tuple/struct/enum), klasses (record or list of records)\nReturns: value with updated trait implementations\nDispatches to &record:with-impls, &tuple:with-impls, &struct:with-impls, &enum:with-impls")
          :code $ quote
            defn with-impls (x klass)
              if (struct? x) (&struct:with-impls x klass)
                if (enum? x) (&enum:with-impls x klass)
                  if (record? x) (&record:with-impls x klass)
                    if (tuple? x) (&tuple:with-impls x klass)
                      raise $ str-spaced "|with-impls expects record/tuple/struct/enum, got:" (type-of x)
        |with-traits $ %{} :CodeEntry (:doc "|Append trait implementations\nSyntax: (with-traits value & traits)\nParams: value (record/tuple/struct/enum), traits (record, variadic)\nReturns: value with updated trait implementations\nDispatches to &record:with-traits, &tuple:with-traits, &struct:with-traits, &enum:with-traits")
          :code $ quote
            defn with-traits (x & traits)
              assert "|with-traits expects records as traits" $ every? traits record?
              if (struct? x) (&struct:with-traits x & traits)
                if (enum? x) (&enum:with-traits x & traits)
                  if (record? x) (&record:with-traits x & traits)
                    if (tuple? x) (&tuple:with-traits x & traits)
                      raise $ str-spaced "|with-traits expects record/tuple/struct/enum, got:" (type-of x)
        |&core-list-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! &core-list-methods (:any? any?) (:add append) (:append append) (:assoc &list:assoc) (:assoc-after &list:assoc-after) (:assoc-before &list:assoc-before) (:bind mapcat) (:butlast butlast) (:concat &list:concat) (:contains? &list:contains?) (:includes? &list:includes?) (:count &list:count) (:drop drop) (:each each)
              :empty $ defn &list:empty (x) ([])
              :empty? &list:empty?
              :filter &list:filter
              :filter-not filter-not
              :find find
              :find-index find-index
              :find-last &list:find-last
              :find-last-index &list:find-last-index
              :foldl $ defn method:foldl (xs v0 f) (foldl xs v0 f)
              :get &list:nth
              :get-in get-in
              :group-by group-by
              :index-of index-of
              :join join
              :join-str join-str
              :last-index-of &list:last-index-of
              :map &list:map
              :map-indexed map-indexed
              :mappend $ defn &list:mappend (x y) (&list:concat x y)
              :max &list:max
              :min &list:min
              :nth &list:nth
              :pairs-map pairs-map
              :prepend prepend
              :reduce reduce
              :reverse &list:reverse
              :slice &list:slice
              :sort $ defn method:sort (x y) (sort x y)
              :sort-by &list:sort-by
              :take take
              :take-last take-last
              :to-set &list:to-set
              :first &list:first
              :rest &list:rest
              :dissoc &list:dissoc
              :to-list identity
              :map-pair &list:map-pair
              :filter-pair &list:filter-pair
              :apply $ defn &fn:apply (xs fs)
                &list:concat & $ map fs
                  defn &fn:ap-gen (f)
                    map xs $ defn &fn:ap-gen (x) (f x)
              :flatten &list:flatten
        |&core-map-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! &core-map-methods (:add &map:add-entry) (:assoc &map:assoc) (:common-keys &map:common-keys) (:contains? &map:contains?) (:count &map:count) (:destruct &map:destruct) (:diff-keys &map:diff-keys) (:diff-new &map:diff-new) (:dissoc &map:dissoc)
              :empty $ defn &map:empty (x) (&{})
              :empty? &map:empty?
              :filter &map:filter
              :filter-kv &map:filter-kv
              :get &map:get
              :get-in get-in
              :includes? &map:includes?
              :keys keys
              :map &map:map
              :map-kv map-kv
              :map-list &map:map-list
              :mappend merge
              :merge merge
              :to-list &map:to-list
              :to-map identity
              :to-pairs to-pairs
              :values vals
        |&core-nil-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! &core-nil-methods
              :to-list $ defn &nil:to-list (_) ([])
              :to-map $ defn &nil:to-map (_) (&{})
              :pairs-map $ defn &nil:pairs-map (_) (&{})
              :to-set $ defn &nil:to-set (_) (#{})
              :to-string $ defn &nil:to-string (_) |
              :to-number $ defn &nil:to-number (_) 0
              :map $ defn &nil:map (_ _f) nil
              :filter $ defn &nil:filter (_ _f) nil
              :bind $ defn &nil:bind (_ _f) nil
              :mappend $ defn &nil:mappend (_ x) x
              :apply $ defn &nil:apply (_ _f) nil
        |&core-number-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! &core-number-methods (:ceil ceil)
              :empty $ defn &number:empty (x) 0
              :floor floor
              :format &number:format
              :display-by &number:display-by
              :inc inc
              :pow pow
              :round round
              :round? round?
              :fract &number:fract
              :sqrt sqrt
              :negate negate
              :rem &number:rem
        |&core-set-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! &core-set-methods (:add include) (:contains? &set:includes?) (:count &set:count) (:destruct &set:destruct) (:difference difference)
              :empty $ defn &set:empty (x) (#{})
              :empty? &set:empty?
              :exclude exclude
              :filter &set:filter
              :include include
              :includes? &set:includes?
              :intersection intersection
              :mappend union
              :max &set:max
              :min &set:min
              :to-list &set:to-list
              :to-set identity
              :union union
        |&core-string-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defrecord! &core-string-methods (:blank? blank?) (:count &str:count)
              :empty $ defn &str:empty (_) |
              :ends-with? ends-with?
              :get &str:nth
              :parse-float parse-float
              :replace &str:replace
              :split split
              :split-lines split-lines
              :starts-with? starts-with?
              :strip-prefix strip-prefix
              :strip-suffix strip-suffix
              :slice &str:slice
              :trim trim
              :empty? &str:empty?
              :contains? &str:contains?
              :includes? &str:includes?
              :nth &str:nth
              :first &str:first
              :rest &str:rest
              :pad-left &str:pad-left
              :pad-right &str:pad-right
              :find-index &str:find-index
              :get-char-code get-char-code
              :escape &str:escape
              :mappend &str:concat
        |Show $ %{} :CodeEntry (:doc "|Core trait: Show")
          :code $ quote
            deftrait Show
              :show (:: :fn ('T) ('T) :string)
        |Eq $ %{} :CodeEntry (:doc "|Core trait: Eq")
          :code $ quote
            deftrait Eq
              :eq? (:: :fn ('T) ('T 'T) :bool)
        |Add $ %{} :CodeEntry (:doc "|Core trait: Add")
          :code $ quote
            deftrait Add
              :add (:: :fn ('T) ('T 'T) 'T)
        |Multiply $ %{} :CodeEntry (:doc "|Core trait: Multiply")
          :code $ quote
            deftrait Multiply
              :multiply (:: :fn ('T) ('T 'T) 'T)
        |Len $ %{} :CodeEntry (:doc "|Core trait: Len")
          :code $ quote
            deftrait Len
              :len (:: :fn ('T) ('T) :number)
        |&core-show-impl $ %{} :CodeEntry (:doc "|Core trait impl for Show")
          :code $ quote
            defrecord! &core-show-impl
              :show &str
        |&core-eq-impl $ %{} :CodeEntry (:doc "|Core trait impl for Eq")
          :code $ quote
            defrecord! &core-eq-impl
              :eq? &=
        |&core-add-number-impl $ %{} :CodeEntry (:doc "|Core trait impl for Add on number")
          :code $ quote
            defrecord! &core-add-number-impl
              :add &+
        |&core-add-string-impl $ %{} :CodeEntry (:doc "|Core trait impl for Add on string")
          :code $ quote
            defrecord! &core-add-string-impl
              :add &str:concat
        |&core-add-list-impl $ %{} :CodeEntry (:doc "|Core trait impl for Add on list")
          :code $ quote
            defrecord! &core-add-list-impl
              :add &list:concat
        |&core-multiply-number-impl $ %{} :CodeEntry (:doc "|Core trait impl for Multiply on number")
          :code $ quote
            defrecord! &core-multiply-number-impl
              :multiply &*
        |&core-len-list-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on list")
          :code $ quote
            defrecord! &core-len-list-impl
              :len &list:count
        |&core-len-map-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on map")
          :code $ quote
            defrecord! &core-len-map-impl
              :len &map:count
        |&core-len-set-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on set")
          :code $ quote
            defrecord! &core-len-set-impl
              :len &set:count
        |&core-len-string-impl $ %{} :CodeEntry (:doc "|Core trait impl for Len on string")
          :code $ quote
            defrecord! &core-len-string-impl
              :len &str:count
        |&core-number-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for number")
          :code $ quote
            def &core-number-impls $ [] &core-number-methods &core-show-impl &core-eq-impl &core-add-number-impl &core-multiply-number-impl
        |&core-string-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for string")
          :code $ quote
            def &core-string-impls $ [] &core-string-methods &core-show-impl &core-eq-impl &core-add-string-impl &core-len-string-impl
        |&core-list-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for list")
          :code $ quote
            def &core-list-impls $ [] &core-list-methods &core-show-impl &core-eq-impl &core-add-list-impl &core-len-list-impl
        |&core-map-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for map")
          :code $ quote
            def &core-map-impls $ [] &core-map-methods &core-show-impl &core-eq-impl &core-len-map-impl
        |&core-set-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for set")
          :code $ quote
            def &core-set-impls $ [] &core-set-methods &core-show-impl &core-eq-impl &core-len-set-impl
        |&core-nil-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for nil")
          :code $ quote
            def &core-nil-impls $ [] &core-nil-methods &core-show-impl &core-eq-impl
        |&core-fn-impls $ %{} :CodeEntry (:doc "|Built-in implementation list for fn")
          :code $ quote
            def &core-fn-impls $ [] &core-fn-methods &core-show-impl
        |&doseq $ %{} :CodeEntry (:doc "|Side-effect traversal macro. Iterates over a binding pair, executing the body for each element and returning nil.")
          :code $ quote
            defmacro &doseq (pair & body)
              if
                not $ and (list? pair)
                  &= 2 $ &list:count pair
                raise $ str-spaced "|doseq expects a pair, got:" pair
              let
                  name $ &list:first pair
                  xs0 $ last pair
                quasiquote $ foldl ~xs0 nil
                  defn doseq-fn% (_acc ~name) ~@body
          :examples $ []
            quote $ do
              defatom *seen ([])
              &doseq (n ([] 1 2))
                reset! *seen $ append (deref *seen) n
              assert= ([] 1 2) $ deref *seen
        |&field-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &field-match-internal (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Warn] field-match found no matched case, missing `_` case?" ~value
                &let
                  pair $ first body
                  if
                    not $ list? pair
                    raise $ str-spaced "|field-match expected arm in list, got:" pair
                  let
                      pattern $ &list:nth pair 0
                    assert "|expected literal or symbol as tag" $ or (tag? pattern) (symbol? pattern)
                    if (&= pattern '_)
                      &let ()
                        assert "|field-match expected a branch after `_`" $ &= 2 (&list:count pair)
                        if
                          not $ &= 1 (&list:count body)
                          eprintln "|[Warn] expected `_` beginning last branch of field-match"
                        &list:nth pair 1
                      &let ()
                        assert "|field-match expected an with (tag new-name body)" $ &= 3 (&list:count pair)
                        quasiquote $ if
                          &= ~pattern $ &map:get ~value :tag
                          &let
                              ~ $ &list:nth pair 1
                              , ~value
                            ~ $ &list:nth pair 2
                          &field-match-internal ~value $ ~@ (&list:rest body)
        |&init-builtin-impls! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &init-builtin-impls! () (; "this function to make sure builtin impls are loaded") (identity &core-number-impls) (identity &core-string-impls) (identity &core-set-impls) (identity &core-list-impls) (identity &core-map-impls) (identity &core-nil-impls) (identity &core-fn-impls)
        |&list-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &list-match-internal (v branch1 pair branch2)
              quasiquote $ if (empty? ~v)
                &let () ~@branch1
                &let
                    ~ $ first pair
                    &list:nth ~v 0
                  &let
                      ~ $ &list:nth pair 1
                      &list:slice ~v 1
                    &let () ~@branch2
        |&list:filter $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:filter (xs f)
              hint-fn $ return-type :list
              assert-type xs :list
              assert-type f :fn
              reduce xs ([])
                defn %&list:filter (acc x)
                  if (f x) (append acc x) acc
        |&list:filter-pair $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:filter-pair (xs f)
              hint-fn $ return-type :list
              assert-type f :fn
              if (list? xs)
                &list:filter xs $ defn %filter-pair (pair)
                  assert "|expected a pair" $ and (list? pair)
                    = 2 $ count pair
                  f (nth pair 0) (nth pair 1)
                raise $ str-spaced "|expected list or map from `filter-pair`, got:" xs
        |&list:find-last $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:find-last (xs f)
              foldr-shortcut xs nil nil $ fn (_acc x)
                if (f x) (:: true x) (:: false nil)
        |&list:find-last-index $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:find-last-index (xs f)
              foldr-shortcut xs
                dec $ count xs
                , nil $ fn (idx x)
                  if (f x) (:: true idx)
                    :: false $ &- 1 idx
        |&list:flatten $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:flatten (xs)
              if (list? xs)
                &list:concat & $ map xs &list:flatten
                [] xs
        |&list:last-index-of $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:last-index-of (xs item)
              foldr-shortcut xs
                dec $ count xs
                , nil $ fn (idx x)
                  if (&= item x) (:: true idx)
                    :: false $ &- 1 idx
        |&list:map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:map (xs f)
              hint-fn $ return-type :list
              assert-type xs :list
              assert-type f :fn
              foldl xs ([])
                defn %&list:map (acc x)
                  append acc $ f x
        |&list:map-pair $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:map-pair (xs f)
              if (list? xs)
                map xs $ defn %map-pair (pair)
                  assert "|expected a pair" $ and (list? pair)
                    = 2 $ count pair
                  f (nth pair 0) (nth pair 1)
                raise $ str-spaced "|expected list or map from `map-pair`, got:" xs
        |&list:max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:max (xs)
              list-match xs
                () nil
                (x0 xss)
                  reduce xss x0 $ defn %max (acc x) (&max acc x)
        |&list:min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:min (xs)
              list-match xs
                () nil
                (x0 xss)
                  reduce xss x0 $ defn %min (acc x) (&min acc x)
        |&list:sort-by $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &list:sort-by (xs f)
              if (tag? f)
                sort xs $ defn %&list:sort-by (a b)
                  &compare (get a f) (get b f)
                sort xs $ defn %&list:sort-by (a b)
                  &compare (f a) (f b)
        |&map:add-entry $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:add-entry (xs pair)
              hint-fn $ return-type :map
              assert-type xs :map
              assert-type pair :list
              assert "|&map:add-entry expected value in a pair" $ and (list? pair)
                &= 2 $ count pair
              &map:assoc xs (nth pair 0) (nth pair 1)
        |&map:filter $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:filter (xs f)
              hint-fn $ return-type :map
              assert-type xs :map
              assert-type f :fn
              reduce xs (&{})
                defn %&map:filter (acc x)
                  if (f x)
                    &map:assoc acc (nth x 0) (nth x 1)
                    , acc
        |&map:filter-kv $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:filter-kv (xs f)
              hint-fn $ return-type :map
              assert-type xs :map
              assert-type f :fn
              reduce xs (&{})
                defn %map:filter-kv (acc x)
                  if
                    f (nth x 0) (nth x 1)
                    &map:assoc acc (nth x 0) (nth x 1)
                    , acc
        |&map:map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:map (xs f)
              hint-fn $ return-type :map
              assert-type xs :map
              assert-type f :fn
              foldl xs ({})
                defn &map:map (acc pair)
                  &let
                    result $ f pair
                    assert "|expected pair returned when mapping hashmap" $ and (list? result)
                      &= 2 $ &list:count result
                    &map:assoc acc (nth result 0) (nth result 1)
        |&map:map-list $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &map:map-list (xs f)
              hint-fn $ return-type :list
              assert-type f :fn
              if (map? xs)
                foldl xs ([])
                  defn %&map:map-list (acc pair)
                    append acc $ f pair
                raise $ str-spaced "|&map:map-list expected a map, got:" xs
        |&max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &max (a b)
              assert "|expects numbers for &max" $ if (number? a) (number? b)
              if (&> a b) a b
        |&min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &min (a b)
              assert "|expects numbers for &min" $ if (number? a) (number? b)
              if (&< a b) a b
        |&record-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &record-match-internal (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Warn] record-match found no matched case, missing `_` case?" ~value
                &let
                  pair $ &list:nth body 0
                  if
                    not $ list? pair
                    raise $ str-spaced "|record-match expected arm in list, got:" pair
                  let
                      pattern $ &list:nth pair 0
                    assert "|expected record or symbol as pattern" $ or (record? pattern) (symbol? pattern)
                    if (&= pattern '_)
                      &let ()
                        assert "|record-match expected a branch after `_`" $ &<= 3 (&list:count pair)
                        quasiquote $ &let
                            ~ $ &list:nth pair 1
                            , ~value
                          ~@ $ &list:slice pair 2
                      &let ()
                        assert "|record-match expected an with (proto new-name & body)" $ &<= 3 (&list:count pair)
                        quasiquote $ if (&record:matches? ~pattern ~value)
                          &let
                              ~ $ &list:nth pair 1
                              , ~value
                            ~@ $ &list:slice pair 2
                          &record-match-internal ~value $ ~@ (&list:rest body)
        |&set:filter $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &set:filter (xs f)
              hint-fn $ return-type :set
              assert-type xs :set
              assert-type f :fn
              reduce xs (#{})
                defn %&set:filter (acc x)
                  if (f x) (&include acc x) acc
        |&set:max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &set:max (xs)
              &let
                pair $ &set:destruct xs
                if (nil? pair) nil $ reduce (nth pair 1) (nth pair 0)
                  defn %max (acc x) (&max acc x)
        |&set:min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn &set:min (xs)
              &let
                pair $ &set:destruct xs
                if (nil? pair) nil $ reduce (nth pair 1) (nth pair 0)
                  defn %min (acc x) (&min acc x)
        |&str-spaced $ %{} :CodeEntry (:doc "|Internal function for joining strings with spaces, used by str-spaced")
          :code $ quote
            defn &str-spaced (head? x0 & xs)
              if (&list:empty? xs)
                if head? (&str x0)
                  if (nil? x0) | $ &str:concat "| " x0
                if (some? x0)
                  &str:concat
                    if head? (&str x0) (&str:concat "| " x0)
                    &str-spaced false & xs
                  &str-spaced head? & xs
        |&tag-match-internal $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro &tag-match-internal (value t & body)
              if (&list:empty? body)
                quasiquote $ raise (str-spaced "|tag-match found no matched case, missing `_` for" ~value)
                &let
                  pair $ &list:first body
                  if
                    not $ and (list? pair)
                      &= 2 $ &list:count pair
                    raise $ str-spaced "|tag-match expected pairs, got:" pair
                  let
                      pattern $ &list:nth pair 0
                      branch $ &list:nth pair 1
                    if (list? pattern)
                      &let
                        k $ &list:first pattern
                        &let
                          size $ &list:count pattern
                          quasiquote $ if
                            if (identical? ~t ~k) (identical? ~size (&tuple:count ~value)) false
                            let
                              ~ $ map-indexed (&list:rest pattern)
                                defn %tag-match (idx x)
                                  [] x $ quasiquote
                                    &tuple:nth ~value $ ~ (inc idx)
                              , ~branch
                            &tag-match-internal ~value ~t $ ~@ (&list:rest body)
                      if (&= pattern '_) branch $ raise (str-spaced "|unknown supported pattern:" pair)
        |* $ %{} :CodeEntry (:doc "|Multiply numbers together")
          :code $ quote
            defn * (x & ys)
              hint-fn $ return-type :number
              assert-type x :number
              assert-type ys $ :: :& :number
              reduce ys x &*
          :examples $ []
            quote $ assert= 6 $ * 2 3
            quote $ assert= 24 $ * 2 3 4
            quote $ assert= 2 $ * 2
            quote $ assert= 24 $ * 2 3 4
            quote $ assert= 30 $ * 5 6
            quote $ assert= 1 $ * 1
        |+ $ %{} :CodeEntry (:doc "|Mathematical addition operation\nFunction: Calculates the sum of one or more numbers\nParams: x (number), ys (variadic args, list of numbers)\nReturns: number - sum of all arguments\nNotes: Supports any number of arguments, requires at least one argument")
          :code $ quote
            defn + (x & ys)
              assert-type x :number
              assert-type ys $ :: :& :number
              reduce ys x &+
          :examples $ []
            quote $ assert= 6 $ + 1 2 3
            quote $ assert= 15 $ + 5 10
        |- $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn - (x & ys)
              hint-fn $ return-type :number
              assert-type x :number
              assert-type ys $ :: :& :number
              if (&list:empty? ys) (&- 0 x) (reduce ys x &-)
          :examples $ []
            quote $ assert= 5 $ - 10 3 2
            quote $ assert= -5 $ - 5
        |-> $ %{} :CodeEntry (:doc "|Thread-first macro\nSyntax: (-> value step1 step2 ...)\nEvaluates the value through each step by inserting it as the first argument and returns the final result.")
          :code $ quote
            defmacro -> (base & xs)
              if (&list:empty? xs) (quasiquote ~base)
                &let
                  x0 $ &list:first xs
                  if (list? x0)
                    recur
                      &list:concat
                        [] (&list:first x0) base
                        &list:rest x0
                      , & $ &list:rest xs
                    recur ([] x0 base) & $ &list:rest xs
          :examples $ []
            quote $ assert= 3 $ -> 1 inc inc
            quote $ assert= 9 $ -> 2 inc (* 3)
        |thread-first $ %{} :CodeEntry (:doc "|a alias for `->`")
          :code $ quote
            defmacro thread-first (& xs)
              quasiquote $ -> ~@xs
        |->% $ %{} :CodeEntry (:doc "|pass value as `%` into several expressions")
          :code $ quote
            defmacro ->% (base & xs)
              if (&list:empty? xs) base $ let
                  tail $ last xs
                  pairs $ &list:concat
                    [] $ [] '% base
                    map (butlast xs)
                      defn %->% (x) ([] '% x)
                quasiquote $ let ~pairs ~tail
        |thread-as $ %{} :CodeEntry (:doc "|a alias for `->%`")
          :code $ quote
            defmacro thread-as (& xs)
              quasiquote $ ->% ~@xs
        |->> $ %{} :CodeEntry (:doc "|thread macro passing value at end of each expression")
          :code $ quote
            defmacro ->> (base & xs)
              if (&list:empty? xs) (quasiquote ~base)
                &let
                  x0 $ &list:first xs
                  if (list? x0)
                    &call-spread recur (append x0 base) & $ &list:rest xs
                    &call-spread recur ([] x0 base) & $ &list:rest xs
        |thread-last $ %{} :CodeEntry (:doc "|a alias for `->>`")
          :code $ quote
            defmacro thread-last (& xs)
              quasiquote $ ->> ~@xs
        |/ $ %{} :CodeEntry (:doc |dividing)
          :code $ quote
            defn / (x & ys)
              hint-fn $ return-type :number
              assert-type x :number
              assert-type ys $ :: :& :number
              if (&list:empty? ys) (&/ 1 x) (reduce ys x &/)
          :examples $ []
            quote $ / 12 3 2
            quote $ / 8
        |/= $ %{} :CodeEntry (:doc "|not equal")
          :code $ quote
            defn /= (a b) (not= a b)
        |: $ %{} :CodeEntry (:doc "|Macro sugar for tagged tuples\nExpands to `::` while passing the tag through `turn-tag`, so both keywords and bare symbols may be used.")
          :code $ quote
            defmacro : (tag & args)
              quasiquote $ ::
                ~ $ turn-tag tag
                ~@ args
          :examples $ []
            quote $ assert= (:: :point 1 2) $ : :point 1 2
            quote $ assert= (:: :name |calcit) $ : |name |calcit
        |;nil $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro ;nil (& _body) nil
        |< $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn < (x & ys)
              hint-fn $ return-type :bool
              assert-type x :number
              assert-type ys $ :: :& :number
              if
                &= 1 $ &list:count ys
                &< x $ &list:first ys
                foldl-compare ys x &<
        |<- $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro <- (& xs)
              quasiquote $ ->
                ~@ $ reverse xs
        |<= $ %{} :CodeEntry (:doc "|Less than or equal comparison, supports multiple arguments")
          :code $ quote
            defn <= (x & ys)
              hint-fn $ return-type :bool
              assert-type x :number
              assert-type ys $ :: :& :number
              if
                &= 1 $ &list:count ys
                &<= x $ &list:first ys
                foldl-compare ys x &<=
          :examples $ []
            quote $ assert= true $ <= 3 5
            quote $ assert= true $ <= 3 3
            quote $ assert= true $ <= 1 2 3 4
        |= $ %{} :CodeEntry (:doc "|Equality predicate for one or more values\nReturns true only when every provided argument is equal, short-circuiting on the first mismatch.")
          :code $ quote
            defn = (x & ys)
              hint-fn $ return-type :bool
              if
                &= 1 $ &list:count ys
                &= x $ &list:first ys
                foldl-compare ys x &=
          :examples $ []
            quote $ assert= true $ = 3 3 3
            quote $ assert= false $ = 1 2
            quote $ assert= true $ = ([] 1 2) ([] 1 2)
        |> $ %{} :CodeEntry (:doc "|Greater-than comparison for one or more numbers\nReturns true only when the value strictly decreases across every argument.")
          :code $ quote
            defn > (x & ys)
              hint-fn $ return-type :bool
              assert-type x :number
              assert-type ys $ :: :& :number
              if
                &= 1 $ &list:count ys
                &> x $ &list:first ys
                foldl-compare ys x &>
          :examples $ []
            quote $ assert= true $ > 5 3
            quote $ assert= false $ > 3 5
            quote $ assert= true $ > 8 4 2 1
            quote $ assert= false $ > 2 2
        |>= $ %{} :CodeEntry (:doc "|Greater-than-or-equal comparison for one or more numbers")
          :code $ quote
            defn >= (x & ys)
              hint-fn $ return-type :bool
              assert-type x :number
              assert-type ys $ :: :& :number
              if
                &= 1 $ &list:count ys
                &>= x $ &list:first ys
                foldl-compare ys x &>=
          :examples $ []
            quote $ assert= true $ >= 5 3
            quote $ assert= true $ >= 5 5
            quote $ assert= false $ >= 3 5
        |[,] $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro [,] (& body)
              &let
                xs $ &list:filter body
                  fn (x) (/= x ',)
                quasiquote $ [] ~@xs
        |[][] $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro [][] (& xs)
              &let
                items $ map xs
                  fn (ys)
                    quasiquote $ [] ~@ys
                quasiquote $ [] ~@items
        |\ $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro \ (& xs)
              quasiquote $ defn %\ (? % %2) ~xs
        |\. $ %{} :CodeEntry (:doc "|this syntax is bared used, deprecating")
          :code $ quote
            defmacro \. (args-alias & xs)
              &let
                args $ ->% (turn-string args-alias) (split % |,) (map % turn-symbol)
                &let
                  inner-body $ if
                    &= 1 $ &list:count xs
                    &list:first xs
                    quasiquote $ &let () ~@xs
                  apply-args (inner-body args)
                    fn (body ys)
                      if (&list:empty? ys) (quasiquote ~body)
                        &let
                          a0 $ last ys
                          &let
                            code $ [] (quasiquote defn)
                              turn-symbol $ &str:concat |f_ (turn-string a0)
                              [] a0
                              , body
                            recur code $ butlast ys
        |and $ %{} :CodeEntry (:doc "|Logical conjunction macro with short-circuit semantics\nReturns the first falsy value or the last truthy value, evaluating expressions left to right.")
          :code $ quote
            defmacro and (item & xs)
              if (&list:empty? xs)
                if (list? item)
                  &let
                    v1# $ gensym |v1
                    quasiquote $ &let (~v1# ~item) (if ~v1# ~v1# false)
                  quasiquote $ if ~item ~item false
                quasiquote $ if ~item
                  and
                    ~ $ &list:first xs
                    ~@ $ &list:rest xs
                  , false
          :examples $ []
            quote $ assert= false $ and true false true
            quote $ assert= |done $ and true |done
        |any? $ %{} :CodeEntry (:doc "|checks if any element in collection satisfies the predicate function, returns true on first match, short-circuits evaluation")
          :code $ quote
            defn any? (xs f)
              hint-fn $ return-type :bool
              assert-type f :fn
              foldl-shortcut xs false false $ defn %any? (acc x)
                if (f x) (:: true true) (:: false acc)
          :examples $ []
            quote $ assert= true $ any? ([] 1 2 3 4) even?
            quote $ assert= false $ any? ([] 1 3 5 7) even?
            quote $ assert= false $ any? ([]) even?
            quote $ assert= false $ any? ([] 1 2 3) $ fn (x) (> x 10)
            quote $ assert= true $ any? ([] 5 15 25) $ fn (x) (> x 10)
        |apply $ %{} :CodeEntry (:doc "|calls a function with arguments from a list, spreads the list as individual arguments")
          :code $ quote
            defn apply (f args)
              assert-type f :fn
              assert-type args :list
              f & args
          :examples $ []
            quote $ assert= 6 $ apply + ([] 1 2 3)
            quote $ assert= 10 $ apply * ([] 2 5)
            quote $ assert= |abc $ apply str ([] |a |b |c)
        |apply-args $ %{} :CodeEntry (:doc "|macro that applies a function to arguments, handles empty argument list specially")
          :code $ quote
            defmacro apply-args (args f)
              if
                &= [] $ &list:first args
                quasiquote $ ~f
                  ~@ $ &list:rest args
                quasiquote $ ~f ~@args
          :examples $ []
            quote $ assert= 6 $ apply-args ([] 1 2 3) +
            quote $ assert= 15 $ apply-args ([] 5 10) +
        |assert $ %{} :CodeEntry (:doc "|asserts that an expression is truthy, raises an error with message if not")
          :code $ quote
            defmacro assert (message xs)
              if
                if (string? xs)
                  not $ string? message
                  , false
                quasiquote $ assert ~xs ~message
                quasiquote $ &let ()
                  if
                    not $ string? ~message
                    raise $ str-spaced "|expects 1st argument to be string, got:" ~message
                  if ~xs nil $ &let ()
                    eprintln "|Failed assertion:" $ format-to-lisp (quote ~xs)
                    raise $ ~
                      &str:concat (&str:concat message "| ") (format-to-lisp xs)
          :examples $ []
            quote $ assert "|x should be positive" (> 1 0)
            quote $ assert "|list should not be empty" (not (empty? $ [] 1))
        |even? $ %{} :CodeEntry (:doc "|check if number is even?")
          :code $ quote
            defn even? (n)
              hint-fn $ return-type :bool
              assert-type n :number
              &= 0 $ &number:rem n 2
        |assert-detect $ %{} :CodeEntry (:doc "|asserts that a value satisfies a predicate function, raises error with details if not")
          :code $ quote
            defmacro assert-detect (f code)
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~code)
                  if (~f ~v) nil $ &let () (eprintln)
                    eprintln
                      format-to-lisp $ quote ~code
                      , "|does not satisfy:"
                        format-to-lisp $ quote ~f
                        , "| <--------"
                    eprintln "|  value is:" ~v
                    raise "|Not satisfied in assertion!"
          :examples $ []
            quote $ assert-detect number? (+ 1 2)
            quote $ assert-detect list? ([] 1 2 3)
            quote $ assert-detect even? (* 2 5)
        |assert= $ %{} :CodeEntry (:doc "|asserts that two values are equal, raises error showing both values if not")
          :code $ quote
            defmacro assert= (a b)
              &let
                va $ gensym |va
                &let
                  vb $ gensym |vb
                  quasiquote $ &let (~va ~a)
                    &let (~vb ~b)
                      if (not= ~va ~vb)
                        &let () (eprintln) (eprintln "|Left: " ~va)
                          eprintln "|      " $ format-to-lisp (quote ~a)
                          eprintln |Right: ~vb
                          eprintln "|      " $ format-to-lisp (quote ~b)
                          raise "|not equal in assertion!"
          :examples $ []
            quote $ assert= 4 (+ 2 2)
            quote $ assert= |hello (str |hel |lo)
            quote $ assert= ([] 1 2 3) (range 1 4)
        |assoc $ %{} :CodeEntry (:doc "|associates a key-value pair to a collection, works on maps, lists, tuples, and records")
          :code $ quote
            defn assoc (x & args)
              if (nil? x)
                raise $ str-spaced "|assoc does not work on nil for:" args
                if (tuple? x) (&tuple:assoc x & args)
                  if (list? x) (&list:assoc x & args)
                    if (record? x) (&record:assoc x & args) (.assoc x & args)
          :examples $ []
            quote $ assert= (&{} :a 1 :b 2) $ assoc (&{} :a 1) :b 2
            quote $ assert= ([] 10 2 3) $ assoc ([] 1 2 3) 0 10
            quote $ assert= (&{} :a 1 :b 3) $ assoc (&{} :a 1 :b 2) :b 3
        |assoc-in $ %{} :CodeEntry (:doc "|associates a value at a nested path in a data structure, creates intermediate maps if needed")
          :code $ quote
            defn assoc-in (data path v)
              list-match path
                () v
                (p0 ps)
                  &let
                    d $ either data (&{})
                    assoc d p0 $ assoc-in
                      if (contains? d p0) (get d p0) (&{})
                      , ps v
          :examples $ []
            quote $ assert= (&{} :a (&{} :b 1)) $ assoc-in (&{}) ([] :a :b) 1
            quote $ assert= (&{} :a (&{} :b 2)) $ assoc-in (&{} :a (&{} :b 1)) ([] :a :b) 2
            quote $ assert= (&{} :x (&{} :y (&{} :z 3))) $ assoc-in (&{}) ([] :x :y :z) 3
        |bool? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn bool? (x)
              &= (type-of x) :bool
        |buffer? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn buffer? (x)
              &= (type-of x) :buffer
        |call-w-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro call-w-log (f & xs)
              let
                  v $ if
                    = :eval $ &get-calcit-running-mode
                    gensym |v
                    , '_log_tmp
                  args-value $ gensym |args-value
                quasiquote $ let
                    ~args-value $ [] ~@xs
                    ~v $ ~f & ~args-value
                  println |call:
                    format-to-lisp $ quote (call-w-log ~f ~@xs)
                    , |=> ~v
                  println "|f:   " ~f
                  println |args: ~args-value
                  ~ v
        |call-wo-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro call-wo-log (f & xs)
              quasiquote $ ~f ~@xs
        |case $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro case (item & patterns)
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~item) (&case ~v nil ~@patterns)
        |case-default $ %{} :CodeEntry (:doc "|Case macro variant with an explicit default branch\nEvaluates the target once, compares it against pattern/result pairs, and falls back to the provided default when no pattern matches.")
          :code $ quote
            defmacro case-default (item default & patterns)
              if (&list:empty? patterns)
                raise $ str-spaced "|Expected patterns for case-default, got empty after:" default
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~item) (&case ~v ~default ~@patterns)
          :examples $ []
            quote $ assert= |two $ case-default 2 |none (1 |one) (2 |two)
            quote $ assert= |none $ case-default 3 |none (1 |one)
            quote $ assert= |fallback $ case-default 5 |fallback (1 |one) (2 |two) (3 |three)
        |concat $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn concat (& args)
              hint-fn $ return-type :list
              list-match args
                () $ []
                (a0 as)
                  do
                    assert-type a0 :list
                    .concat a0 & as
          :examples $ []
            quote $ assert= ([] 1 2 3 4 5) $ concat ([] 1 2) ([] 3 4) ([] 5)
        |cond $ %{} :CodeEntry (:doc "|Multi-branch conditional macro. Evaluates condition/result pairs in order and returns the first truthy branch; use `true` as a default guard.")
          :code $ quote
            defmacro cond (pair & else)
              if
                not $ and (list? pair)
                  &= 2 $ &list:count pair
                raise $ str-spaced "|expects a pair, got:" pair
              &let
                expr $ &list:nth pair 0
                &let
                  branch $ &list:nth pair 1
                  if
                    if (empty? else) (= true expr) false
                    , branch $ quasiquote
                      if ~expr ~branch $ ~
                        if (&list:empty? else) nil $ quasiquote
                          cond
                            ~ $ &list:nth else 0
                            ~@ $ &list:rest else
          :examples $ []
            quote $ assert= :small $ cond
              (&< 2 1) :nope
              (&< 2 5) :small
              true :fallback
            quote $ assert= :fallback $ cond
              false :branch
              true :fallback
        |conj $ %{} :CodeEntry (:doc "|Appends values to the end of a list, returning a new list\nSupports adding multiple values by chaining additional arguments.")
          :code $ quote
            defn conj (xs y0 & ys)
              hint-fn $ return-type :list
              assert-type xs :list
              if (empty? ys) (append xs y0)
                recur (append xs y0) & ys
          :examples $ []
            quote $ assert= ([] 1 2 3) $ conj ([] 1 2) 3
            quote $ assert= ([] 1 2 3 4) $ conj ([] 1) 2 3 4
        |contains-in? $ %{} :CodeEntry (:doc "|Checks whether a nested path exists within maps, records, tuples, or lists. Returns true only when every hop succeeds.")
          :code $ quote
            defn contains-in? (xs path)
              list-match path
                () true
                (p0 ps)
                  cond
                      list? xs
                      if
                        and (number? p0) (&list:contains? xs p0)
                        recur (nth xs p0) ps
                        , false
                    (map? xs)
                      if (&map:contains? xs p0)
                        recur (&map:get xs p0) ps
                        , false
                    (record? xs)
                      if (&record:contains? xs p0)
                        recur (&record:get xs p0) ps
                        , false
                    (tuple? xs)
                      if
                        and (&>= p0 0)
                          &< p0 $ &tuple:count xs
                        recur (&tuple:nth xs p0) ps
                        , false
                    true false
          :examples $ []
            quote $ assert= true $ contains-in?
              {} (:profile ({} (:name |calcit)))
              [] :profile :name
            quote $ assert= false $ contains-in?
              {} (:profile ({} (:name |calcit)))
              [] :profile :missing
        |contains-symbol? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn contains-symbol? (xs y)
              if (list? xs)
                apply-args (xs)
                  defn %contains-symbol? (body)
                    list-match body
                      () false
                      (b0 bs)
                        if (contains-symbol? b0 y) true $ recur bs
                &= xs y
        |contains? $ %{} :CodeEntry (:doc "|Checks whether a collection contains a key or index at the current level. Supports lists, tuples, maps, and records while treating nil as false.")
          :code $ quote
            defn contains? (x k)
              hint-fn $ return-type :bool
              if (nil? x) false $ if (list? x) (&list:contains? x k)
                if (record? x) (&record:contains? x k)
                  if (tuple? x)
                    and (&>= k 0)
                      &< k $ &tuple:count x
                    .contains? x k
          :examples $ []
            quote $ assert= true $ contains? ([] :a :b) 1
            quote $ assert= true $ contains? ({} (:a 1)) :a
            quote $ assert= false $ contains? nil :missing
            quote $ assert= true $ contains? (#{} 1 2 3) 2
        |count $ %{} :CodeEntry (:doc "|Counts elements in a collection or string\nNil input returns 0; otherwise delegates to the underlying data structure's counter.")
          :code $ quote
            defn count (x)
              hint-fn $ return-type :number
              if (nil? x) 0 $ if (tuple? x) (&tuple:count x)
                if (list? x) (&list:count x)
                  if (record? x) (&record:count x) (.count x)
          :examples $ []
            quote $ assert= 4 $ count ([] 1 2 3 4)
            quote $ assert= 5 $ count |hello
            quote $ assert= 0 $ count nil
        |dec $ %{} :CodeEntry (:doc "|Decrements a number by 1")
          :code $ quote
            defn dec (x)
              hint-fn $ return-type :number
              assert-type x :number
              &- x 1
          :examples $ []
            quote $ assert= 4 $ dec 5
            quote $ assert= -1 $ dec 0
            quote $ assert= -4 $ dec -3
        |def $ %{} :CodeEntry (:doc "|special macro to expose value to definition")
          :code $ quote
            defmacro def (_name x) x
        |defn-w-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro defn-w-log (f-name args & body)
              quasiquote $ defn ~f-name ~args
                &let
                  ~f-name $ defn ~f-name ~args ~@body
                  call-w-log ~f-name ~@args
        |defn-wo-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro defn-wo-log (f-name args & body)
              quasiquote $ defn ~f-name ~args ~@body
        |defrecord $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro defrecord (name & xs)
              quasiquote $ new-record
                ~ $ turn-tag name
                , ~@xs
        |defrecord! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro defrecord! (name & pairs)
              quasiquote $ %{}
                new-record
                  ~ $ turn-tag name
                  ~@ $ map pairs &list:first
                , ~@pairs
        |defstruct $ %{} :CodeEntry (:doc "|macro for defining record structs\nSyntax: (defstruct Name [('T 'S)] :field :type ...)\nParams: Name (symbol/tag), optional generics list, field pairs (tag + type)\nReturns: struct definition value\nExpands to &struct::new")
          :code $ quote
            defmacro defstruct (name & pairs)
              assert "|defstruct expects name as tag/symbol" $ or (tag? name) (symbol? name)
              assert "|defstruct expects pairs in list" $ and (list? pairs) (every? pairs list?)
              &let
                first-pair $ if
                  empty? pairs
                  , nil
                  &list:first pairs
                &let
                  generics $ if
                    list? first-pair
                    if
                      every? first-pair
                        fn (item)
                          if
                            list? item
                            if
                              &= 2 $ count item
                              &= 'quote $ &list:first item
                              , false
                            , false
                      first-pair
                      []
                    []
                  &let
                    field-pairs $ if
                      empty? generics
                      , pairs
                      &list:rest pairs
                    assert "|defstruct expects (field type) pairs" $ every? field-pairs
                      fn (pair)
                        &let
                          items $ if
                            &= [] $ &list:first pair
                            &list:rest pair
                            , pair
                          &= 2 $ count items
                    &let
                      normalized $ map field-pairs
                        fn (pair)
                          &let
                            items $ if
                              &= [] $ &list:first pair
                              &list:rest pair
                              , pair
                            quasiquote $ [] ~@items
                      if
                        empty? generics
                        quasiquote $ &struct::new
                          ~ $ turn-tag name
                          ~@ normalized
                        quasiquote $ &struct::new
                          ~ $ turn-tag name
                          ~ generics
                          ~@ normalized
          :examples $ []
            quote $ defstruct Person (:name :string) (:age :number)
        |defenum $ %{} :CodeEntry (:doc "|macro for defining enums\nSyntax: (defenum Name :variant type... ...)\nParams: Name (symbol/tag), variants (tag + payload types)\nReturns: enum prototype value\nExpands to &enum::new")
          :code $ quote
            defmacro defenum (name & variants)
              assert "|defenum expects name as tag/symbol" $ or (tag? name) (symbol? name)
              assert "|defenum expects variants in list" $ and (list? variants) (every? variants list?)
              assert "|defenum expects (variant & payloads)" $ every? variants
                fn (variant)
                  &let
                    items $ if
                      &= [] $ &list:first variant
                      &list:rest variant
                      , variant
                    &>= (count items) 1
              &let
                normalized $ map variants
                  fn (variant)
                    &let
                      items $ if
                        &= [] $ &list:first variant
                        &list:rest variant
                        , variant
                      quasiquote $ [] ~@items
                quasiquote $ &enum::new
                  ~ $ turn-tag name
                  ~@ normalized
          :examples $ []
            quote $ defenum Result (:ok :number) (:err :string)
        |deftrait $ %{} :CodeEntry (:doc "|macro for defining traits\nSyntax: (deftrait Name (:method (:: :fn (args...) return)) ...)\nParams: Name (symbol/tag), methods (list of (tag type))\nNotes: use :fn (tag) for DynFn when signature is intentionally omitted\nReturns: trait definition value\nExpands to &trait::new")
          :code $ quote
            defmacro deftrait (name & methods)
              assert "|deftrait expects (method type) pairs" $ every? methods list?
              &let
                normalize-trait-type $ fn (t0)
                  if (list? t0)
                    &let
                      size $ &list:count t0
                      &let
                        head $ &list:first t0
                        &let
                          second $ if
                            &>= size 2
                            &list:nth t0 1
                            , nil
                          &let
                            third $ if
                              &>= size 3
                              &list:nth t0 2
                              , nil
                            &let
                              wrap-list $ fn (x)
                                if (list? x) x ([] x)
                              if
                                and (tag? head) (&= head :fn) (&= size 3)
                                [] :fn ([]) (wrap-list second) (&list:nth t0 2)
                                if
                                  and (tag? head) (&= head :fn) (&= size 4)
                                  [] :fn (wrap-list second) (wrap-list third) (&list:nth t0 3)
                                  if
                                    and (tag? second) (&= second :fn) (&= size 4)
                                    [] head second ([]) (wrap-list third) (&list:nth t0 3)
                                    if
                                      and (tag? second) (&= second :fn) (&= size 5)
                                      [] head second (wrap-list third) (wrap-list (&list:nth t0 3)) (&list:nth t0 4)
                                      , t0
                    , t0
                &let
                  normalized $ map methods
                    fn (entry)
                      &let
                        items $ if
                          &= [] $ &list:first entry
                          &list:rest entry
                          , entry
                        assert "|deftrait expects (method type) pairs" $ &= 2 $ count items
                        let
                            m0 $ &list:first items
                            t0 $ &list:nth items 1
                            t1 $ normalize-trait-type t0
                          quasiquote $ [] ~m0 (quote ~t1)
                  quasiquote
                    def ~name
                      &trait::new
                        ~ $ turn-tag name
                        [] ~@normalized
        |destruct-list $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-list (xs)
              if (empty? xs) (:: :none)
                :: :some (nth xs 0) (&list:slice xs 1)
        |destruct-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-map (xs)
              &let
                pair $ &map:destruct xs
                if (nil? pair) (:: :none) (:: :some & pair)
        |destruct-set $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-set (xs)
              &let
                pair $ &set:destruct xs
                if (nil? pair) (:: :none)
                  :: :some (nth pair 0) (nth pair 1)
        |destruct-str $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn destruct-str (s)
              if (&= s |) (:: :none)
                :: :some (nth s 0) (&str:slice s 1)
        |difference $ %{} :CodeEntry (:doc "|Returns the set difference of base and all other sets")
          :code $ quote
            defn difference (base & xs)
              hint-fn $ return-type :set
              assert-type base :set
              assert-type xs $ :: :& :set
              reduce xs base $ fn (acc item) (&difference acc item)
          :examples $ []
            quote $ assert= (#{} 1) $ difference (#{} 1 2 3) (#{} 2 3 4)
            quote $ assert= (#{} 1 2) $ difference (#{} 1 2 3 4) (#{} 3 4 5)
        |dissoc $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn dissoc (x & args)
              if (nil? x) nil $ if (list? x) (&list:dissoc x & args) (.dissoc x & args)
        |dissoc-in $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn dissoc-in (data path)
              list-match path
                () nil
                (p0 ps)
                  if
                    &= 1 $ &list:count path
                    dissoc data p0
                    assoc data p0 $ dissoc-in (get data p0) ps
        |distinct $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn distinct (x)
              hint-fn $ return-type :list
              assert-type x :list
              &list:distinct x
        |do $ %{} :CodeEntry (:doc "|Evaluates expressions sequentially and returns the last result\nUseful for grouping side effects or multiple steps where only the final value matters.")
          :code $ quote
            defmacro do (& body)
              ; println |body: $ format-to-lisp body
              if (empty? body) (raise "|empty do is not okay")
              quasiquote $ &let () (~@ body)
          :examples $ []
            quote $ assert= 3 $ do (inc 1) (+ 1 2)
            quote $ assert= |world $ do (str |hello) (str |world)
        |drop $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn drop (xs n)
              hint-fn $ return-type :list
              assert-type n :number
              slice xs n $ &list:count xs
        |each $ %{} :CodeEntry (:doc "|Iterate over a collection and apply function f for side effects, returns nil")
          :code $ quote
            defn each (xs f)
              assert-type f :fn
              foldl xs nil $ defn %each (_acc x) (f x)
          :examples $ []
            quote
              assert= nil
                each ([] 1 2 3) $ fn (x) (&+ x 1)
        |either $ %{} :CodeEntry (:doc "|Returns the first non-nil value among its arguments\nBehaves like a nil-coalescing macro: only nil triggers evaluation of subsequent branches, so false is preserved as a value.")
          :code $ quote
            defmacro either (item & xs)
              if (&list:empty? xs) item $ if (list? item)
                &let
                  v1# $ gensym |v1
                  quasiquote $ &let (~v1# ~item)
                    if (nil? ~v1#)
                      either
                        ~ $ &list:first xs
                        ~@ $ &list:rest xs
                      ~ v1#
                quasiquote $ if (nil? ~item)
                  either
                    ~ $ &list:first xs
                    ~@ $ &list:rest xs
                  ~ item
          :examples $ []
            quote $ assert= 42 $ either nil 42 nil
            quote $ assert= false $ either false true
            quote $ assert= |backup $ either nil nil |backup
        |empty $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn empty (x)
              if (nil? x) nil $ if (list? x) ([]) (.empty x)
        |empty? $ %{} :CodeEntry (:doc "|Checks whether a collection or string is empty\nNil values are considered empty, otherwise delegates to the underlying data structure.")
          :code $ quote
            defn empty? (x)
              hint-fn $ return-type :bool
              if (nil? x) true $ if (list? x) (&list:empty? x) (.empty? x)
          :examples $ []
            quote $ assert= true $ empty? ([])
            quote $ assert= false $ empty? ([] 1)
        |ends-with? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn ends-with? (x y)
              hint-fn $ return-type :bool
              assert-type x :string
              assert-type y :string
              &=
                &- (&str:count x) (&str:count y)
                &str:find-index x y
        |every? $ %{} :CodeEntry (:doc "|Checks whether every element of a collection satisfies a predicate, short-circuiting on the first failure.")
          :code $ quote
            defn every? (xs f)
              hint-fn $ return-type :bool
              assert-type f :fn
              foldl-shortcut xs true true $ defn %every? (acc x)
                if (f x) (:: false acc) (:: true false)
          :examples $ []
            quote $ assert= true $ every? ([] 2 4 6) $ defn %even (x) (&= 0 (.rem x 2))
            quote $ assert= false $ every? ([] 1 2 3) $ defn %gt1 (x) (&> x 1)
        |exclude $ %{} :CodeEntry (:doc "|Removes values from a collection by repeatedly calling `&exclude` for each provided item.")
          :code $ quote
            defn exclude (base & xs)
              hint-fn $ return-type :set
              assert-type base :set
              assert-type xs $ :: :& :set
              reduce xs base $ fn (acc item) (&exclude acc item)
        |field-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro field-match (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Error] field-match expected patterns for matching" ~value
                if (list? value)
                  &let
                    v# $ gensym |v
                    quasiquote $ &let (~v# ~value)
                      assert "|expected map value to match" $ map? ~v#
                      &field-match-internal ~v# ~@body
                  quasiquote $ &let ()
                    assert "|expected map value to match" $ map? ~value
                    &field-match-internal ~value ~@body
        |filter $ %{} :CodeEntry (:doc "|Builds a new collection containing only the elements where the predicate returns truthy, preserving the original collection type when possible.")
          :code $ quote
            defn filter (xs f)
              assert-type f :fn
              .filter xs f
          :examples $ []
            quote $ assert= ([] 2 4) $ filter ([] 1 2 3 4 5) $ defn %even? (x) (&= 0 (.rem x 2))
            quote $ assert= ([] |bb |ccc) $ filter ([] |a |bb |ccc) $ defn %long? (s) (&> (&str:count s) 1)
        |filter-not $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn filter-not (xs f)
              assert-type f :fn
              .filter xs $ defn %filter-not (x)
                not $ f x
        |find $ %{} :CodeEntry (:doc "|Find the first element in a collection that satisfies the predicate f")
          :code $ quote
            defn find (xs f)
              assert-type f :fn
              foldl-shortcut xs 0 nil $ defn %find (_acc x)
                if (f x) (:: true x) (:: false nil)
          :examples $ []
            quote $ assert= 4 $ find ([] 1 2 3 4 5) $ fn (x) (&> x 3)
            quote $ assert= nil $ find ([] 1 2 3) $ fn (x) (&> x 10)
        |find-index $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn find-index (xs f)
              hint-fn $ return-type :number
              assert-type f :fn
              foldl-shortcut xs 0 nil $ defn %find-index (idx x)
                if (f x) (:: true idx)
                  :: false $ &+ 1 idx
        |first $ %{} :CodeEntry (:doc "|Returns the first element of a list, tuple, string, or other sequential structure\nNil inputs return nil, and empty collections also produce nil.")
          :code $ quote
            defn first (x)
              if (nil? x) nil $ if (tuple? x) (&tuple:nth x 0)
                if (list? x) (&list:nth x 0) (.first x)
          :examples $ []
            quote $ assert= 1 $ first ([] 1 2 3)
            quote $ assert= |h $ first |hello
            quote $ assert= nil $ first nil
        |flipped $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro flipped (f & args)
              quasiquote $ ~f
                ~@ $ reverse args
        |fn $ %{} :CodeEntry (:doc "|macro for anonymous functions\nSyntax: (fn (args...) body...)\nParams: args (parameter list), body (expressions)\nReturns: anonymous function\nCreates an anonymous function, shorter than defn")
          :code $ quote
            defmacro fn (args & body)
              quasiquote $ defn f% ~args ~@body
          :examples $ []
            quote $ map ([] 1 2 3) $ fn (x) (* x 2)
            quote $ filter ([] 1 2 3 4 5) $ fn (n) (> n 2)
        |fn? $ %{} :CodeEntry (:doc "|Check if a value is a function")
          :code $ quote
            defn fn? (x)
              hint-fn $ return-type :bool
              if
                &= (type-of x) :fn
                , true $ &= (type-of x) :proc
          :examples $ []
            quote $ assert= true $ fn? inc
            quote $ assert= false $ fn? 123
            quote $ assert= false $ fn? |text
        |foldl' $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn foldl' (xs acc f)
              list-match xs
                () acc
                (x0 xss)
                  recur xss (f acc x0) f
        |foldl-compare $ %{} :CodeEntry (:doc "|Helper used by comparison operators to ensure a relation holds across an entire list, short-circuiting on the first failure.")
          :code $ quote
            defn foldl-compare (xs acc f)
              if (&list:empty? xs) true $ if
                f acc $ &list:first xs
                recur (&list:rest xs) (&list:first xs) f
                , false
          :examples $ []
            quote $ assert= true $ foldl-compare ([] 1 2 3 4) 0 &<
            quote $ assert= false $ foldl-compare ([] 1 3 2 4) 0 &<
        |frequencies $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn frequencies (xs0)
              assert "|expects a list for frequencies" $ list? xs0
              apply-args
                  {}
                  , xs0
                fn (acc xs)
                  list-match xs
                    () acc
                    (x0 xss)
                      recur
                        if (contains? acc x0)
                          update acc x0 $ \ &+ % 1
                          &map:assoc acc x0 1
                        , xss
        |get $ %{} :CodeEntry (:doc "|Reads a value from collections or strings by key or index. Handles maps, lists, tuples, records, and strings; nil bases return nil.")
          :code $ quote
            defn get (base k)
              if (nil? base) nil $ if (string? base) (&str:nth base k)
                if (map? base) (&map:get base k)
                  if (list? base) (&list:nth base k)
                    if (tuple? base) (&tuple:nth base k)
                      if (record? base) (&record:get base k)
                        raise $ str-spaced "|Expected map or list for get, got:" base k
          :examples $ []
            quote $ assert= 2 $ get ([] 0 2 4) 1
            quote $ assert= |b $ get |abc 1
            quote $ assert= nil $ get nil :missing
        |get-in $ %{} :CodeEntry (:doc "|Get value from nested data structure using a path of keys")
          :code $ quote
            defn get-in (base path)
              if
                not $ list? path
                raise $ str-spaced "|expects path in a list, got:" path
              if (nil? base) nil $ list-match path
                () base
                (y0 ys)
                  recur (get base y0) ys
          :examples $ []
            quote $ assert= 1 $ get-in ({} (:a ({} (:b 1)))) ([] :a :b)
            quote $ assert= 2 $ get-in ([] ([] 1 2) ([] 3 4)) ([] 0 1)
            quote $ assert= nil $ get-in ({} (:x |value)) ([] :y)
        |group-by $ %{} :CodeEntry (:doc "|Group elements by the result of applying function f to each element")
          :code $ quote
            defn group-by (xs0 f)
              apply-args
                  {}
                  , xs0
                defn %group-by (acc xs)
                  list-match xs
                    () acc
                    (x0 xss)
                      let
                          key $ f x0
                        recur
                          if (contains? acc key)
                            update acc key $ \ append % x0
                            &map:assoc acc key $ [] x0
                          , xss
        |identity $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn identity (x) x
        |if-let $ %{} :CodeEntry (:doc "|Conditionally binds the result of an expression to a symbol and executes the matching branch when the value is non-nil.")
          :code $ quote
            defmacro if-let (pair then ? else)
              if
                not $ and (list? pair)
                  &= 2 $ count pair
                raise $ str-spaced "|expected a pair, got:" pair
              &let
                x $ nth pair 0
                if
                  not $ symbol? x
                  raise $ str-spaced "|expected a symbol for var name, got:" x
                quasiquote $ &let
                  ~x $ ~ (nth pair 1)
                  if (some? ~x) ~then ~else
          :examples $ []
            quote $ assert= |found $ if-let (v (:: :some |found)) v |missing
            quote $ assert= |missing $ if-let (v (:: :none)) v |missing
        |if-not $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro if-not (condition true-branch ? false-branch)
              quasiquote $ if ~condition ~false-branch ~true-branch
        |inc $ %{} :CodeEntry (:doc "|Increments a number by 1")
          :code $ quote
            defn inc (x)
              hint-fn $ return-type :number
              &+ x 1
          :examples $ []
            quote $ assert= 6 $ inc 5
            quote $ assert= 1 $ inc 0
        |include $ %{} :CodeEntry (:doc "|Add elements to a set, returns a new set with the elements included")
          :code $ quote
            defn include (base & xs)
              hint-fn $ return-type :set
              assert-type base :set
              assert-type xs $ :: :& :set
              reduce xs base $ fn (acc item) (&include acc item)
          :examples $ []
            quote $ assert= (#{} 1 2 3 4) $ include (#{} 1 2) 3 4
            quote $ assert= (#{} 1 2 3) $ include (#{} 1 2) 2 3
        |includes? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn includes? (x k)
              hint-fn $ return-type :bool
              if (nil? x) false $ if (list? x) (&list:includes? x k) (.includes? x k)
        |index-of $ %{} :CodeEntry (:doc "|Find the first index of an item in a list, returns nil if not found")
          :code $ quote
            defn index-of (xs item)
              hint-fn $ return-type :number
              foldl-shortcut xs 0 nil $ defn %index-of (idx x)
                if (&= item x) (:: true idx)
                  :: false $ &+ 1 idx
          :examples $ []
            quote $ assert= 1 $ index-of ([] |a |b |c) |b
            quote $ assert= nil $ index-of ([] 1 2 3) 5
        |interleave $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn interleave (xs0 ys0)
              hint-fn $ return-type :list
              apply-args
                  []
                  , xs0 ys0
                defn %interleave (acc xs ys)
                  if
                    if (&list:empty? xs) true $ &list:empty? ys
                    , acc $ recur
                      -> acc
                        append $ &list:first xs
                        append $ &list:first ys
                      rest xs
                      rest ys
        |intersection $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn intersection (base & xs)
              hint-fn $ return-type :set
              assert-type base :set
              assert-type xs $ :: :& :set
              reduce xs base $ fn (acc item) (&set:intersection acc item)
        |join $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn join (xs0 sep)
              hint-fn $ return-type :list
              apply-args
                  []
                  , xs0 true
                defn %join (acc xs beginning?)
                  list-match xs
                    () acc
                    (x0 xss)
                      recur
                        append
                          if beginning? acc $ append acc sep
                          , x0
                        , xss false
        |join-str $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn join-str (xs0 sep)
              hint-fn $ return-type :string
              assert-type xs0 :list
              assert-type sep :string
              apply-args (| xs0 true)
                defn %join-str (acc xs beginning?)
                  list-match xs
                    () acc
                    (x0 xss)
                      recur
                        &str:concat
                          if beginning? acc $ &str:concat acc sep
                          , x0
                        , xss false
        |js-object $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro js-object (& xs)
              &let
                ys $ &list:concat & xs
                quasiquote $ &js-object ~@ys
        |keys $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn keys (x)
              hint-fn $ return-type :set
              map (to-pairs x) &list:first
        |keys-non-nil $ %{} :CodeEntry (:doc "|Get keys from a map that have non-nil values")
          :code $ quote
            defn keys-non-nil (x)
              hint-fn $ return-type :set
              apply-args
                  #{}
                  to-pairs x
                fn (acc pairs)
                  if (&set:empty? pairs) acc $ &let
                    set-pair $ &set:destruct pairs
                    &let
                      pair $ nth set-pair 0
                      if
                        nil? $ last pair
                        recur acc $ nth set-pair 1
                        recur
                          include acc $ &list:first pair
                          nth set-pair 1
          :examples $ []
            quote $ assert= (#{} :a :b) $ keys-non-nil ({} (:a 1) (:b 2) (:c nil))
            quote $ assert= (#{}) $ keys-non-nil ({} (:a nil) (:b nil))
        |last $ %{} :CodeEntry (:doc "|Returns the last element of a list-like collection\nReturns nil when the collection is empty.")
          :code $ quote
            defn last (xs)
              if (empty? xs) nil $ nth xs
                &- (count xs) 1
          :examples $ []
            quote $ assert= 3 $ last ([] 1 2 3)
            quote $ assert= nil $ last ([])
        |let $ %{} :CodeEntry (:doc "|macro for local bindings\nSyntax: (let ([name value] ...) body...)\nParams: pairs (list of binding pairs), body (expressions)\nReturns: result of body with bindings in scope\nCreates multiple local bindings sequentially")
          :code $ quote
            defmacro let (pairs & body)
              if
                not $ and (list? pairs) (every? pairs list?)
                raise $ str-spaced "|expects pairs in list for let, got:" pairs
              if
                &= 1 $ &list:count pairs
                quasiquote $ &let
                  ~ $ &list:nth pairs 0
                  ~@ body
                if (&list:empty? pairs)
                  quasiquote $ &let () ~@body
                  quasiquote $ &let
                    ~ $ &list:nth pairs 0
                    let
                      ~ $ &list:rest pairs
                      ~@ body
          :examples $ []
            quote $ let ((x 1) (y 2)) (+ x y)
            quote $ let ((a 10)) (* a a)
        |let-destruct $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro let-destruct (pattern v & body)
              if (symbol? pattern)
                quasiquote $ &let (~pattern ~v) ~@body
                if (list? pattern)
                  if
                    &= [] $ &list:first pattern
                    quasiquote $ let[]
                      ~ $ &list:rest pattern
                      , ~v ~@body
                    if
                      &= '{} $ &list:first pattern
                      quasiquote $ let{}
                        ~ $ &list:rest pattern
                        , ~v ~@body
                      raise $ str-spaced "|Unknown pattern to destruct:" pattern
                  raise $ str-spaced "|Unknown structure to destruct:" pattern
        |let-sugar $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro let-sugar (pairs & body)
              if
                not $ and (list? pairs) (every? pairs list?)
                raise $ str-spaced "|expects pairs in list for let, got:" pairs
              if (&list:empty? pairs)
                quasiquote $ &let () ~@body
                &let
                  pair $ &list:first pairs
                  if
                    not $ &= 2 (&list:count pair)
                    raise $ str-spaced "|expected pair length of 2, got:" pair
                  if
                    &= 1 $ &list:count pairs
                    quasiquote $ let-destruct ~@pair (~@ body)
                    quasiquote $ let-destruct ~@pair
                      let-sugar
                        ~ $ &list:rest pairs
                        ~@ body
        |let[] $ %{} :CodeEntry (:doc "|Destructures a sequential value inside `let`, assigning each position to declared names and supporting `&` rest bindings.")
          :code $ quote
            defmacro let[] (vars data & body)
              if
                not $ and (list? vars)
                  every? vars $ fn (x)
                    or
                      symbol? x
                      is-spreading-mark? x
                raise $ str-spaced "|expects a list of definitions, got:" vars
              let
                  variable? $ symbol? data
                  v $ if variable? data (gensym |v)
                  defs $ apply-args
                    [] ([]) vars 0
                    defn let[]% (acc xs idx)
                      if (&list:empty? xs) acc $ &let ()
                        if
                          not $ or
                            symbol? (&list:first xs)
                            is-spreading-mark? (&list:first xs)
                          raise $ &str:concat "|Expected symbol for vars: " (&list:first xs)
                        if
                          is-spreading-mark? (&list:first xs)
                          &let ()
                            assert "|expected list spreading" $ &= 2 (&list:count xs)
                            append acc $ [] (&list:nth xs 1)
                              quasiquote $ &list:slice ~v ~idx
                          recur
                            append acc $ [] (&list:first xs)
                              quasiquote $ &list:nth ~v ~idx
                            rest xs
                            inc idx
                if variable?
                  quasiquote $ let (~ defs) (~@ body)
                  quasiquote $ &let (~v ~data)
                    let (~ defs) (~@ body)
          :examples $ []
            quote $ let[] (x y) ([] 1 2) (+ x y)
            quote $ let[] (head & tail) ([] 9 8 7) (count tail)
        |let{} $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro let{} (items base & body)
              if
                not $ and (list? items) (every? items symbol?)
                raise $ str-spaced "|expects symbol names in binding names, got:" items
              &let
                var-result $ gensym |result
                quasiquote $ &let (~var-result ~base)
                  assert (str "|expected map for destructing: " ~var-result) (map? ~var-result)
                  let
                    ~ $ map items
                      defn gen-items% (x)
                        [] x $ [] (turn-tag x) var-result
                    ~@ body
        |list-match $ %{} :CodeEntry (:doc "|Two-branch list destructuring macro. Provides separate clauses for the empty list and a head/tail pattern, useful for simple recursion or guards.")
          :code $ quote
            defmacro list-match (xs pattern1 pattern2)
              assert "|patterns in list" $ and (list? pattern1) (list? pattern2)
                &> (count pattern1) 1
                list? $ &list:nth pattern1 0
                list? $ &list:nth pattern2 0
                &> (count pattern2) 1
              &let
                v# $ gensym |v
                quasiquote $ &let (~v# ~xs)
                  if
                    not $ list? ~v#
                    raise "|expected a list in list-match"
                  ~ $ if
                    and
                      empty? $ &list:nth pattern1 0
                      &= 2 $ count (&list:nth pattern2 0)
                    quasiquote $ &list-match-internal ~v#
                      ~ $ &list:slice pattern1 1
                      ~ $ &list:nth pattern2 0
                      ~ $ &list:slice pattern2 1
                    if
                      and
                        empty? $ &list:nth pattern2 0
                        &= 2 $ count (&list:nth pattern1 0)
                      quasiquote $ &list-match-internal ~v#
                        ~ $ &list:slice pattern2 1
                        ~ $ &list:nth pattern1 0
                        ~ $ &list:slice pattern1 1
                      raise "|expected empty and destruction branches"
          :examples $ []
            quote $ assert= :something $ list-match ([] 1)
              () :empty
              (a b) :something
            quote $ assert= 1 $ list-match ([] 1 2 3)
              () nil
              (head tail) head
        |list? $ %{} :CodeEntry (:doc "|checks if value is a list\nSyntax: (list? x)\nParams: x (any)\nReturns: true if x is a list, false otherwise\nType predicate for list data structure")
          :code $ quote
            defn list? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :list
          :examples $ []
            quote $ list? ([] 1 2 3)
            quote $ list? ({})
        |cirru-quote? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn cirru-quote? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :cirru-quote
        |loop $ %{} :CodeEntry (:doc "|Named-let style looping macro. Binds initial values once and uses `recur` to update bindings in a tail-recursive way without stack growth.")
          :code $ quote
            defmacro loop (pairs & body)
              if
                not $ list? pairs
                raise $ str-spaced "|expects pairs in loop, got:" pairs
              if
                not $ every? pairs
                  defn detect-pairs? (x)
                    if (list? x)
                      &= 2 $ &list:count x
                      , false
                raise $ str-spaced "|expects pairs in pairs in loop, got:" pairs
              let
                  args $ map pairs &list:first
                  values $ map pairs last
                assert "|loop requires symbols in pairs" $ every? args symbol?
                quasiquote $ apply (defn generated-loop ~args ~@body) ([] ~@values)
          :examples $ []
            quote $ assert= 6 $ loop
              (total 0)
              (xs ([] 1 2 3))
              if (empty? xs) total
                recur (+ total (first xs)) (rest xs)
        |macro? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn macro? (x)
              &= (type-of x) :macro
        |map $ %{} :CodeEntry (:doc "|Collection mapping function. Applies a function to each element of a list, set, or map, returning a structure of the same shape.")
          :code $ quote
            defn map (xs f)
              assert-type f :fn
              if (list? xs) (&list:map xs f)
                if (set? xs)
                  foldl xs (#{})
                    defn %map (acc x)
                      include acc $ f x
                  if (map? xs) (&map:map xs f)
                    raise $ str-spaced "|expected list or set for map function, got:" xs
          :examples $ []
            quote $ assert= ([] 2 3 4) $ map ([] 1 2 3) inc
            quote $ assert= ([] |1 |2 |3) $ map ([] 1 2 3) str
        |map-indexed $ %{} :CodeEntry (:doc "|Map over a collection with index, f takes index and value")
          :code $ quote
            defn map-indexed (xs f)
              hint-fn $ return-type :list
              assert-type f :fn
              foldl xs ([])
                defn %map-indexed (acc x)
                  append acc $ f (count acc) x
          :examples $ []
            quote $ assert= ([] 10 21 32) $ map-indexed ([] 10 20 30) $ fn (i x) (&+ i x)
            quote $ assert= ([] ([] 0 |a) ([] 1 |b)) $ map-indexed ([] |a |b) $ fn (i x) ([] i x)
        |map-kv $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn map-kv (xs f)
              hint-fn $ return-type :map
              assert-type xs :map
              assert-type f :fn
              foldl xs ({})
                defn %map-kv (acc pair)
                  &let
                    result $ f (nth pair 0) (nth pair 1)
                    if (list? result)
                      do
                        assert "|expected pair returned when mapping hashmap" $ &= 2 (&list:count result)
                        &map:assoc acc (nth result 0) (nth result 1)
                      if
                        or (nil? result) (tuple? result)
                        , acc $ raise (str-spaced "|map-kv expected list or nil, got:" result)
        |map? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a map")
          :code $ quote
            defn map? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :map
          :examples $ []
            quote $ assert= true $ map? $ {} (:a 1)
            quote $ assert= false $ map? ([] 1 2)
        |mapcat $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn mapcat (xs f)
              hint-fn $ return-type :list
              assert-type f :fn
              &list:concat & $ map xs f
        |max $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn max (xs)
              hint-fn $ return-type :number
              assert-type xs :list
              .max xs
        |merge $ %{} :CodeEntry (:doc "|Combines maps left-to-right, with later maps overwriting keys from earlier ones by reducing through `&merge`.")
          :code $ quote
            defn merge (x0 & xs)
              hint-fn $ return-type :map
              assert-type x0 :map
              reduce xs x0 &merge
          :examples $ []
            quote $ assert=
              {} (:a 2) (:b 1)
              merge ({} (:a 1)) ({} (:a 2) (:b 1))
            quote $ assert=
              {} (:a 1) (:b 2) (:c 3)
              merge
                {} (:a 1)
                {} (:b 2)
                {} (:c 3)
        |merge-non-nil $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn merge-non-nil (x0 & xs)
              hint-fn $ return-type :map
              assert-type x0 :map
              reduce xs x0 &merge-non-nil
        |min $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn min (xs)
              hint-fn $ return-type :number
              assert-type xs :list
              .min xs
        |negate $ %{} :CodeEntry (:doc "|Negate a number, returns its opposite")
          :code $ quote
            defn negate (x)
              hint-fn $ return-type :number
              assert-type x :number
              &- 0 x
          :examples $ []
            quote $ assert= -5 $ negate 5
            quote $ assert= 3 $ negate -3
            quote $ assert= 0 $ negate 0
        |abs $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn abs (x)
              hint-fn $ return-type :number
              assert-type x :number
              if (&< x 0) (&- 0 x) x
        |nil? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is nil")
          :code $ quote
            defn nil? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :nil
          :examples $ []
            quote $ assert= true $ nil? nil
            quote $ assert= false $ nil? 0
        |non-nil! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn non-nil! (x)
              if (nil? x)
                raise "|expected non nil value"
                , x
        |not= $ %{} :CodeEntry (:doc "|Returns true when its two arguments are not identical according to `=`.")
          :code $ quote
            defn not= (x y)
              hint-fn $ return-type :bool
              not $ &= x y
          :examples $ []
            quote $ assert= true $ not= 1 2
            quote $ assert= false $ not= :a :a
            quote $ assert= true $ not= |a |b
        |noted $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro noted (_doc v) v
        |nth $ %{} :CodeEntry (:doc "|Returns the element at index `i` from a list, tuple, or sequential data structure\nRaises if the index is outside the available range.")
          :code $ quote
            defn nth (x i)
              assert-type i :number
              if (tuple? x) (&tuple:nth x i)
                if (list? x) (&list:nth x i) (.nth x i)
          :examples $ []
            quote $ assert= 2 $ nth ([] 1 2 3) 1
            quote $ assert= |b $ nth |abc 1
        |number? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a numeric scalar")
          :code $ quote
            defn number? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :number
          :examples $ []
            quote $ assert= true $ number? 123
            quote $ assert= true $ number? 3.14
            quote $ assert= false $ number? |text
        |optionally $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn optionally (s)
              if (nil? s) (:: :none) (:: :some s)
        |or $ %{} :CodeEntry (:doc "|Logical disjunction macro. Skips evaluating later forms once a truthy (non-nil, non-false) value is found, preserving the first truthy result.")
          :code $ quote
            defmacro or (item & xs)
              if (&list:empty? xs) item $ if (list? item)
                &let
                  v1# $ gensym |v1
                  quasiquote $ &let (~v1# ~item)
                    if (nil? ~v1#)
                      or
                        ~ $ &list:first xs
                        ~@ $ &list:rest xs
                      if (= false ~v1#)
                        or
                          ~ $ &list:first xs
                          ~@ $ &list:rest xs
                        ~ v1#
                quasiquote $ if (nil? ~item)
                  or
                    ~ $ &list:first xs
                    ~@ $ &list:rest xs
                  if (= false ~item)
                    or
                      ~ $ &list:first xs
                      ~@ $ &list:rest xs
                    ~ item
          :examples $ []
            quote $ assert= |done $ or nil |done false
            quote $ assert= false $ or false nil
            quote $ assert= 2 $ or nil 2 3
        |pairs-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn pairs-map (xs)
              reduce xs ({})
                defn %pairs-map (acc pair)
                  assert "|expects pair for pairs-map" $ if (list? pair)
                    &= 2 $ &list:count pair
                    , false
                  &map:assoc acc (&list:first pair) (last pair)
        |range-bothway $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn range-bothway (x ? y)
              assert-type x :number
              assert-type y $ :: :optional :number
              if (some? y)
                do
                  assert-type y :number
                  range
                    inc $ &- (&+ x x) y
                    , y
                range
                  inc $ negate x
                  , x
        |record-match $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro record-match (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Error] record-match expected patterns for matching" ~value
                if (list? value)
                  &let
                    v# $ gensym |v
                    quasiquote $ &let (~v# ~value)
                      assert "|expected record to match" $ record? ~v#
                      &record-match-internal ~v# ~@body
                  quasiquote $ &let ()
                    assert "|expected record to match" $ record? ~value
                    &record-match-internal ~value ~@body
        |record? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a record created with `new-record` or `defrecord`.")
          :code $ quote
            defn record? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :record
          :examples $ []
            quote $ assert= true $ record? (new-record :point (:x 1) (:y 2))
            quote $ assert= false $ record? ({} (:x 1))
        |struct? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a struct definition.")
          :code $ quote
            defn struct? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :struct
          :examples $ []
            quote $ assert= true $ struct? (defstruct Person (:name :string))
            quote $ assert= false $ struct? (new-record :point (:x 1))
        |enum? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is an enum definition.")
          :code $ quote
            defn enum? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :enum
          :examples $ []
            quote $ assert= true $ enum? (defenum Result (:ok) (:err :string))
            quote $ assert= false $ enum? (:: :ok 1)
        |reduce $ %{} :CodeEntry (:doc "|Collection reduction operation\nFunction: Reduces a collection using a specified function, accumulating elements onto an initial value\nParams: xs (collection), x0 (initial accumulator value), f (reduction function that takes accumulator and current element)\nReturns: any type - final accumulated result\nNotes: The reduction function f should accept two parameters (accumulator, current element) and return a new accumulator value")
          :code $ quote
            defn reduce (xs x0 f) (foldl xs x0 f)
          :examples $ []
            quote $ assert= 6 $ reduce ([] 1 2 3) 0 +
        |ref? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn ref? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :ref
        |repeat $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn repeat (x n0)
              hint-fn $ return-type :list
              assert-type n0 :number
              apply-args
                  []
                  , n0
                defn %repeat (acc n)
                  if (&<= n 0) acc $ recur (append acc x) (&- n 1)
        |rest $ %{} :CodeEntry (:doc "|Returns the collection without its first element\nNil input returns nil; lists delegate to &list:rest.")
          :code $ quote
            defn rest (x)
              if (nil? x) nil $ if (list? x) (&list:rest x) (.rest x)
          :examples $ []
            quote $ assert= ([] 2 3) $ rest ([] 1 2 3)
            quote $ assert= nil $ rest nil
        |reverse $ %{} :CodeEntry (:doc "|Reverse the order of elements in a list")
          :code $ quote
            defn reverse (x)
              hint-fn $ return-type :list
              assert-type x :list
              &list:reverse x
          :examples $ []
            quote $ assert= ([] 3 2 1) $ reverse ([] 1 2 3)
            quote $ assert= ([]) $ reverse ([])
        |section-by $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn section-by (xs0 n)
              if (>= n 1)
                apply-args
                    []
                    , xs0
                  fn (acc xs)
                    if
                      &<= (&list:count xs) n
                      if (&list:empty? xs) acc $ append acc xs
                      recur
                        append acc $ take xs n
                        drop xs n
                raise $ str-spaced "|expected positive number, got:" n
        |select-keys $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn select-keys (m xs)
              assert "|expected map for selecting" $ map? m
              foldl xs (&{})
                defn %select-keys (acc k)
                  &map:assoc acc k $ &map:get m k
        |set? $ %{} :CodeEntry (:doc "|Check if a value is a set")
          :code $ quote
            defn set? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :set
          :examples $ []
            quote $ assert= true $ set? (#{} 1 2 3)
            quote $ assert= false $ set? ([] 1 2 3)
            quote $ assert= false $ set? ({} (:a 1))
        |slice $ %{} :CodeEntry (:doc "|Extract a slice from a collection from index n to m")
          :code $ quote
            defn slice (xs n ? m)
              assert-type n :number
              assert-type m $ :: :optional :number
              if (nil? xs) nil $ .slice xs n m
          :examples $ []
            quote $ assert= ([] 2 3) $ slice ([] 1 2 3 4) 1 3
            quote $ assert= ([] 3 4) $ slice ([] 1 2 3 4) 2
        |some-in? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn some-in? (x path)
              if (nil? x) false $ list-match path
                () true
                (k ps)
                  if (map? x)
                    if (contains? x k)
                      recur (get x k) ps
                      , false
                    if (list? x)
                      if (number? k)
                        recur (get x k) ps
                        , false
                      raise $ &str:concat "|Unknown structure for some-in? detection: " x
        |some? $ %{} :CodeEntry (:doc "|Complement of nil?\nReturns true when the value is not nil.")
          :code $ quote
            defn some? (x)
              hint-fn $ return-type :bool
              not $ nil? x
          :examples $ []
            quote $ assert= true $ some? 0
            quote $ assert= false $ some? nil
        |starts-with? $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn starts-with? (x y)
              hint-fn $ return-type :bool
              assert-type x :string
              assert-type y :string
              &= 0 $ &str:find-index x y
        |str $ %{} :CodeEntry (:doc "|converts values to string and concatenates them")
          :code $ quote
            defn str (x0 & xs)
              hint-fn $ return-type :string
              if (&list:empty? xs) (&str x0)
                &str:concat x0 $ str & xs
          :examples $ []
            quote $ assert= |hello $ str |hello
            quote $ assert= |abc $ str |a |b |c
            quote $ assert= |123 $ str 1 2 3
            quote $ assert= "|hello world" $ str |hello "| " |world
        |str-spaced $ %{} :CodeEntry (:doc "|converts values to string and joins them with spaces")
          :code $ quote
            defn str-spaced (& xs)
              hint-fn $ return-type :string
              &str-spaced true & xs
          :examples $ []
            quote $ assert= "|a b c" $ str-spaced |a |b |c
            quote $ assert= "|1 2 3" $ str-spaced 1 2 3
        |string? $ %{} :CodeEntry (:doc "|checks if value is a string")
          :code $ quote
            defn string? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :string
          :examples $ []
            quote $ assert= true $ string? |hello
            quote $ assert= false $ string? 123
            quote $ assert= false $ string? :keyword
        |strip-prefix $ %{} :CodeEntry (:doc "|removes prefix from string if it starts with that prefix, returns original string otherwise")
          :code $ quote
            defn strip-prefix (s piece)
              hint-fn $ return-type :string
              assert-type s :string
              assert-type piece :string
              if (starts-with? s piece)
                &str:slice s $ &str:count piece
                , s
          :examples $ []
            quote $ assert= "| world" $ strip-prefix "|hello world" |hello
            quote $ assert= |abc $ strip-prefix "|prefix-abc" "|prefix-"
            quote $ assert= |hello $ strip-prefix |hello |xyz
        |strip-suffix $ %{} :CodeEntry (:doc "|removes suffix from string if it ends with that suffix, returns original string otherwise")
          :code $ quote
            defn strip-suffix (s piece)
              hint-fn $ return-type :string
              assert-type s :string
              assert-type piece :string
              if (ends-with? s piece)
                &str:slice s 0 $ &- (&str:count s) (&str:count piece)
                , s
          :examples $ []
            quote $ assert= |hello $ strip-suffix "|hello world" "| world"
            quote $ assert= |abc $ strip-suffix "|abc-suffix" |-suffix
            quote $ assert= |hello $ strip-suffix |hello |xyz
        |deref $ %{} :CodeEntry (:doc "|Reads the current value stored in a reference\nSupports Calcit atoms as well as other host structures that implement deref.")
          :code $ quote
            defn deref (*a)
              if (ref? *a) (&atom:deref *a) (.deref *a)
          :examples $ []
            quote $ do
              defatom *state 1
              assert= 1 $ deref *state
            quote $ do
              defatom *counter 0
              reset! *counter 5
              assert= 5 $ deref *counter
        |swap! $ %{} :CodeEntry (:doc "|Atomically updates a reference by applying a function to its current value and storing the result.")
          :code $ quote
            defmacro swap! (a f & args)
              quasiquote $ reset! ~a
                ~f (&atom:deref ~a) ~@args
          :examples $ []
            quote $ do
              defatom *counter 0
              swap! *counter inc
              assert= 1 $ deref *counter
            quote $ do
              defatom *state 1
              swap! *state + 2
              assert= 3 $ deref *state
        |symbol? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a symbol literal (as opposed to strings, keywords, or other data).")
          :code $ quote
            defn symbol? (x)
              &= (type-of x) :symbol
          :examples $ []
            quote $ assert= true $ symbol?
              quote item
            quote $ assert= false $ symbol? |text
        |syntax? $ %{} :CodeEntry (:doc "|detecting syntax element")
          :code $ quote
            defn syntax? (x)
              &= (type-of x) :syntax
        |tag-match $ %{} :CodeEntry (:doc "|Pattern matching on tagged tuples, dispatches based on the first element of the tuple")
          :code $ quote
            defmacro tag-match (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Error] tag-match expected some patterns and matches" ~value
                &let
                  t# $ gensym |tag
                  &let
                    v# $ gensym |v
                    quasiquote $ &let (~v# ~value)
                      if
                        not $ tuple? ~v#
                        raise $ str "|tag-match expected tuple, got" ~v#
                      &let (~t# $ &tuple:nth ~v# 0)
                        &tuple:validate-enum ~v# ~t#
                        &tag-match-internal ~v# ~t# $ ~@ body
          :examples $ []
            quote
              assert= 11
                tag-match (:: :ok 1)
                  (:ok v) (&+ v 10)
                  (:err e) (eprintln e)
            quote
              assert= |got:hello
                tag-match (:: :some |hello)
                  (:some x) (str x |:got)
                  (:none) |nothing

        |tag? $ %{} :CodeEntry (:doc "|Check if a value is a tag (keyword)")
          :code $ quote
            defn tag? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :tag
          :examples $ []
            quote $ assert= true $ tag? :keyword
            quote $ assert= false $ tag? |string
            quote $ assert= false $ tag? 123
        |tagging-edn $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn tagging-edn (data)
              if (list? data) (map data tagging-edn)
                if (map? data)
                  map-kv data $ defn %tagging (k v)
                    []
                      if (string? k) (turn-tag k) k
                      tagging-edn v
                  , data
        |take $ %{} :CodeEntry (:doc "|Take the first n elements from a list")
          :code $ quote
            defn take (xs n)
              hint-fn $ return-type :list
              assert-type xs :list
              assert-type n :number
              if
                >= n $ &list:count xs
                , xs $ slice xs 0 n
          :examples $ []
            quote $ assert= ([] 1 2) $ take ([] 1 2 3 4) 2
            quote $ assert= ([] 1 2 3) $ take ([] 1 2 3) 5
        |take-last $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn take-last (xs n)
              hint-fn $ return-type :list
              assert-type xs :list
              assert-type n :number
              if
                >= n $ &list:count xs
                , xs $ slice xs
                  - (&list:count xs) n
                  &list:count xs
        |tuple? $ %{} :CodeEntry (:doc "|Predicate that checks whether a value is a tuple literal created with the `::` form.")
          :code $ quote
            defn tuple? (x)
              hint-fn $ return-type :bool
              &= (type-of x) :tuple
          :examples $ []
            quote $ assert= true $ tuple? (:: :a :b)
            quote $ assert= false $ tuple? ([] :a :b)
        |turn-str $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn turn-str (x) (turn-string x)
        |union $ %{} :CodeEntry (:doc "|Returns the union of all sets")
          :code $ quote
            defn union (base & xs)
              hint-fn $ return-type :set
              assert-type base :set
              assert-type xs $ :: :& :set
              reduce xs base $ fn (acc item) (&union acc item)
          :examples $ []
            quote $ assert= (#{} 1 2 3 4) $ union (#{} 1 2) (#{} 3 4)
            quote $ assert= (#{} 1 2 3) $ union (#{} 1) (#{} 2) (#{} 3)
        |unselect-keys $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn unselect-keys (m xs)
              assert "|expected map for unselecting" $ map? m
              foldl xs m $ defn %unselect-keys (acc k) (&map:dissoc acc k)
        |update $ %{} :CodeEntry (:doc "|Applies a function to the value at a given key or index, returning a collection with the updated slot.")
          :code $ quote
            defn update (x k f)
              assert-type f :fn
              if (map? x)
                if (contains? x k)
                  assoc x k $ f (&map:get x k)
                  , x
                if (list? x)
                  if (&list:contains? x k)
                    assoc x k $ f (&list:nth x k)
                    , x
                  if (tuple? x)
                    assoc x k $ f (&tuple:nth x k)
                    if (record? x)
                      if (contains? x k)
                        assoc x k $ f (&record:get x k)
                        , x
                      raise $ &str:concat "|Cannot update key on item: " (to-lispy-string x)
          :examples $ []
            quote $ assert= ({} (:count 2)) $ update ({} (:count 1)) :count inc
            quote $ assert= (:: 0 2 2)
              update (:: 0 1 2) 1 inc
            quote $ assert= ({} (:count 1)) $ update ({} (:count 1)) :missing inc
        |update-in $ %{} :CodeEntry (:doc "|Walks a path of keys inside nested maps/lists and applies a function to the value, creating intermediate maps as needed.")
          :code $ quote
            defn update-in (data path f)
              assert-type f :fn
              list-match path
                () $ f data
                (p0 ps)
                  assoc (either data $ {}) p0 $ update-in (get data p0) ps f
          :examples $ []
            quote $ assert=
              {} (:a ({} (:b 2)))
              update-in ({} (:a ({} (:b 1)))) ([] :a :b) inc
            quote $ assert=
              {} (:profile ({} (:visits 1)))
              update-in {} ([] :profile :visits) (fn (_missing) 1)
            quote $ assert=
              {} (:x 10)
              update-in ({} (:x 5)) ([] :x) $ fn (v) (&* v 2)
        |vals $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn vals (x)
              hint-fn $ return-type :set
              map (to-pairs x) last
          :examples $ []
            quote $ assert= ([] 1 2) $ vals ({} (:a 1) (:b 2))
            quote $ assert= ([]) $ vals ({})
        |w-js-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro w-js-log (x)
              if (list? x)
                &let
                  v $ if
                    = :eval $ &get-calcit-running-mode
                    gensym |v
                    , '_log_tmp
                  quasiquote $ &let (~v ~x)
                    js/console.log
                      format-to-lisp $ quote ~x
                      , |=> ~v
                    ~ v
                quasiquote $ &let ()
                  js/console.log
                    format-to-lisp $ quote ~x
                    , |=> ~x
                  ~ x
        |w-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro w-log (x)
              &let
                v $ if
                  = :eval $ &get-calcit-running-mode
                  gensym |v
                  , '_log_tmp
                if (list? x)
                  quasiquote $ &let (~v ~x)
                    println
                      format-to-lisp $ quote ~x
                      , |=> ~v
                    ~ v
                  quasiquote $ &let ()
                    println
                      format-to-lisp $ quote ~x
                      , |=> ~x
                    ~ x
        |when $ %{} :CodeEntry (:doc "|Conditional macro that evaluates its body only when the test expression is truthy, returning the last body value.")
          :code $ quote
            defmacro when (condition & body)
              if
                &= 1 $ &list:count body
                quasiquote $ if ~condition
                  ~ $ nth body 0
                quasiquote $ if ~condition
                  &let () ~@body
          :examples $ []
            quote $ assert= 4 $ when (&> 3 2)
              inc 3
            quote $ assert= nil $ when false
              inc 1
        |when-let $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro when-let (pair & body)
              if
                not $ and (list? pair)
                  &= 2 $ count pair
                raise $ str-spaced "|expected a pair, got:" pair
              &let
                x $ nth pair 0
                if
                  not $ symbol? x
                  raise $ str-spaced "|expected a symbol for var name, got:" x
                quasiquote $ &let
                  ~x $ ~ (nth pair 1)
                  if (some? ~x)
                    do $ ~@ body
        |when-not $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro when-not (condition & body)
              if
                &= 1 $ &list:count body
                quasiquote $ if (not ~condition)
                  ~ $ nth body 0
                quasiquote $ if (not ~condition)
                  &let () ~@body
        |with-cpu-time $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro with-cpu-time (x)
              let
                  started $ gensym |started
                  v $ gensym |v
                quasiquote $ let
                    ~started $ cpu-time
                    ~v ~x
                  println |[cpu-time]
                    format-to-lisp $ quote ~x
                    , |=>
                      .format
                        &- (cpu-time) ~started
                        , 3
                      , |ms
                  ~ v
        |wo-js-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro w-js-log (x) x
        |wo-log $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro wo-log (x) x
        |zipmap $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn zipmap (xs0 ys0)
              apply-args
                  {}
                  , xs0 ys0
                fn (acc xs ys)
                  if
                    if (&list:empty? xs) true $ &list:empty? ys
                    , acc $ recur
                      &map:assoc acc (&list:first xs) (&list:first ys)
                      rest xs
                      rest ys
        |{,} $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro {,} (& body)
              &let
                xs $ &list:filter body
                  defn &{,} (x) (not= x ',)
                quasiquote $ pairs-map
                  section-by ([] ~@xs) 2
        |{} $ %{} :CodeEntry (:doc "|macro for creating hashmaps\nSyntax: ({} (:key value) ...)\nParams: pairs (key-value pairs)\nReturns: hashmap\nCreates a hashmap from key-value pairs")
          :code $ quote
            defmacro {} (& xs)
              if
                not $ every? xs $ fn (pair)
                  and (list? pair)
                    &= 2 $ &list:count pair
                raise $ str "|{} expects pairs of lists with exactly two elements each, got: " xs
              &let
                ys $ &list:concat & xs
                quasiquote $ &{} ~@ys
          :examples $ []
            quote $ {} (:a 1) (:b 2)
            quote $ {} (:name |Alice) (:age 30)
        |' $ %{} :CodeEntry (:doc "|alias for []")
          :code $ quote
            def "'" []
        |record-with $ %{} :CodeEntry (:doc "|macro to extend existing record with new values in pairs, internally using &record:with which takes flattern items")
          :code $ quote
            defmacro record-with (record & pairs)
              ; "check if args are in pairs"
              if
                not $ and (list? pairs)
                  every? pairs $ fn (xs)
                    and (list? xs) (&= 2 $ count xs)
                raise $ str-spaced "|record-with expects a list of pairs (each with exactly two elements), got:" pairs
              ; "call &record:with"
              quasiquote $ &record:with ~record $ ~@ $ &list:concat & pairs

        ; "=== CalcitSyntax Internal Definitions ==="
        |defn $ %{} :CodeEntry (:doc "|internal syntax for defining functions\nSyntax: (defn name [args] body)\nParams: name (symbol), args (list of symbols), body (expression)\nReturns: function definition\nDefines a named function with parameters and body expression")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ defn my-add (p1 p2) (+ p1 p2)
        |defmacro $ %{} :CodeEntry (:doc "|internal syntax for defining macros\nSyntax: (defmacro name [args] body)\nParams: name (symbol), args (list of symbols), body (expression)\nReturns: macro definition\nDefines a macro that transforms code at compile time")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ do
              defmacro identity-macro (x)
                quasiquote ~x
              assert= 4 $ identity-macro (+ 2 2)
        |if $ %{} :CodeEntry (:doc "|internal syntax for conditional expressions\nSyntax: (if condition then-expr else-expr)\nParams: condition (any), then-expr (any), else-expr (any, optional)\nReturns: value of then-expr if condition is truthy, else-expr otherwise\nEvaluates condition and returns appropriate branch")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ if (> x 0) |positive |non-positive
            quote $ if (empty? xs) 0 (count xs)
        |&let $ %{} :CodeEntry (:doc "|internal syntax for local binding (binds only 1 local)\nSyntax: (&let [binding value] body)\nParams: binding (symbol), value (any), body (expression)\nReturns: result of body with binding in scope\nCreates a local binding for a single variable")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ assert= 6 $ &let (x (+ 1 2)) (* x 2)
            quote $ assert= |done $ &let (label |done) label
        |quote $ %{} :CodeEntry (:doc "|internal syntax for turning code into quoted data\nSyntax: (quote expr)\nParams: expr (any code)\nReturns: quoted data structure\nPrevents evaluation and returns code as data")
          :code $ quote &runtime-inplementation
        |quasiquote $ %{} :CodeEntry (:doc "|internal syntax for quasiquote (used inside macros)\nSyntax: (quasiquote expr)\nParams: expr (code with possible unquote)\nReturns: partially quoted structure\nLike quote but allows selective unquoting with ~ and ~@")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ quasiquote $ &+ ~x 1
            quote $ quasiquote $ [] ~x ~@xs
        |gensym $ %{} :CodeEntry (:doc "|internal syntax for generating unique symbols\nSyntax: (gensym) or (gensym prefix)\nParams: prefix (string, optional)\nReturns: unique symbol\nGenerates a unique symbol for macro hygiene")
          :code $ quote &runtime-inplementation
        |eval $ %{} :CodeEntry (:doc "|internal syntax for evaluating code at runtime\nSyntax: (eval expr)\nParams: expr (quoted code)\nReturns: result of evaluation\nEvaluates quoted code in current environment")
          :code $ quote &runtime-inplementation
        |macroexpand $ %{} :CodeEntry (:doc "|internal syntax for expanding macros until recursive calls are resolved\nSyntax: (macroexpand expr)\nParams: expr (macro call)\nReturns: fully expanded code\nExpands macros recursively until no more macro calls remain")
          :code $ quote &runtime-inplementation
        |macroexpand-1 $ %{} :CodeEntry (:doc "|internal syntax for expanding macro just once for debugging\nSyntax: (macroexpand-1 expr)\nParams: expr (macro call)\nReturns: one-level expanded code\nExpands macro only one level for debugging purposes")
          :code $ quote &runtime-inplementation
        |macroexpand-all $ %{} :CodeEntry (:doc "|internal syntax for expanding macro until macros inside are resolved\nSyntax: (macroexpand-all expr)\nParams: expr (code with macros)\nReturns: fully expanded code\nExpands all macros including nested ones")
          :code $ quote &runtime-inplementation
        |try $ %{} :CodeEntry (:doc "|internal syntax for try-catch error handling\nSyntax: (try body (catch error handler))\nParams: body (expression), error (symbol), handler (expression)\nReturns: result of body or handler if error occurs\nProvides exception handling mechanism")
          :code $ quote &runtime-inplementation
        |defatom $ %{} :CodeEntry (:doc "|internal syntax for defining referenced state\nSyntax: (defatom name initial-value)\nParams: name (symbol), initial-value (any)\nReturns: atom definition\nDefines a mutable reference with initial value")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ ; defatom *my-atom $ {} (:a 1)
        |reset! $ %{} :CodeEntry (:doc "|internal syntax for resetting atom values\nSyntax: (reset! atom new-value)\nParams: atom (atom reference), new-value (any)\nReturns: new value\nSets atom to new value and returns it")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ ; reset! *my-atom $ {} (:a 2)
        |hint-fn $ %{} :CodeEntry (:doc "|internal syntax for function hints (used for async and generics)\nSyntax: (hint-fn hint-data fn-expr)\nParams: hint-data (keyword or list), fn-expr (function)\nReturns: hinted function\nAdds execution hints to functions, including async markers and generic type metadata (type-vars, return-type)")
          :code $ quote &runtime-inplementation
        |&call-spread $ %{} :CodeEntry (:doc "|internal syntax for handling & spreading in function calls\nSyntax: (&call-spread fn args)\nParams: fn (function), args (list with spread)\nReturns: function call result\nHandles argument spreading in function calls")
          :code $ quote &runtime-inplementation
        |& $ %{} :CodeEntry (:doc "|internal syntax for spreading in function definition and call\nSyntax: (& rest-args) in params or (f & args) in calls\nParams: varies based on context\nReturns: varies based on context\nMarks rest parameters or argument spreading")
          :code $ quote &runtime-inplementation
        |? $ %{} :CodeEntry (:doc "|internal syntax for optional argument in function definition\nSyntax: (? optional-arg) in parameter list\nParams: optional-arg (symbol)\nReturns: parameter marker\nMarks optional parameters in function definitions")
          :code $ quote &runtime-inplementation
        |~ $ %{} :CodeEntry (:doc "|internal syntax for interpolating value in macro\nSyntax: (~ expr) inside quasiquote\nParams: expr (expression to evaluate)\nReturns: evaluated expression\nUnquotes expression inside quasiquote")
          :code $ quote &runtime-inplementation
        |~@ $ %{} :CodeEntry (:doc "|internal syntax for spreading interpolate value in macro\nSyntax: (~@ list-expr) inside quasiquote\nParams: list-expr (expression that evaluates to list)\nReturns: spliced list elements\nUnquotes and splices list elements inside quasiquote")
          :code $ quote &runtime-inplementation
        |assert-type $ %{} :CodeEntry (:doc "|internal syntax for type assertion at preprocessing stage\nSyntax: (assert-type expr type-expr)\nParams: expr (any), type-expr (type annotation)\nReturns: evaluated result of expr\nAsserts that expr matches the given type annotation during static analysis")
          :code $ quote &runtime-inplementation

        ; "=== CalcitProc Internal Definitions ==="
        |type-of $ %{} :CodeEntry (:doc "|internal function for getting type of value\nSyntax: (type-of value)\nParams: value (any)\nReturns: tag representing the type\nReturns type tag like :nil, :bool, :number, :string, :list, :map, :set, :fn, etc.")
          :code $ quote &runtime-inplementation
        |recur $ %{} :CodeEntry (:doc "|internal function for tail recursion\nSyntax: (recur args...)\nParams: args (any, variable number)\nReturns: recur structure for tail call optimization\nEnables tail call optimization by marking recursive calls")
          :code $ quote &runtime-inplementation
        |format-to-lisp $ %{} :CodeEntry (:doc "|internal function for formatting to Lisp syntax\nSyntax: (format-to-lisp value)\nParams: value (any)\nReturns: string in Lisp format\nConverts Calcit data structures to Lisp-style string representation")
          :code $ quote &runtime-inplementation
        |format-to-cirru $ %{} :CodeEntry (:doc "|internal function for formatting to Cirru syntax\nSyntax: (format-to-cirru value)\nParams: value (any)\nReturns: string in Cirru format\nConverts Calcit data structures to Cirru-style string representation")
          :code $ quote &runtime-inplementation
        |&reset-gensym-index! $ %{} :CodeEntry (:doc "|internal function for resetting gensym index\nSyntax: (&reset-gensym-index!)\nParams: none\nReturns: nil\nResets the global gensym counter to 0 for deterministic symbol generation")
          :code $ quote &runtime-inplementation
        |&get-calcit-running-mode $ %{} :CodeEntry (:doc "|internal function for getting Calcit running mode\nSyntax: (&get-calcit-running-mode)\nParams: none\nReturns: keyword indicating mode\nReturns current running mode like :dev, :release, or :test")
          :code $ quote &runtime-inplementation
        |generate-id! $ %{} :CodeEntry (:doc "|internal function for generating unique IDs\nSyntax: (generate-id!)\nParams: none\nReturns: unique string ID\nGenerates a unique identifier string for runtime use")
          :code $ quote &runtime-inplementation
        |turn-symbol $ %{} :CodeEntry (:doc "|internal function for converting to symbol\nSyntax: (turn-symbol value)\nParams: value (string, tag, or symbol)\nReturns: symbol\nConverts string, tag, or existing symbol to symbol type")
          :code $ quote &runtime-inplementation
        |turn-tag $ %{} :CodeEntry (:doc "|internal function for converting to tag\nSyntax: (turn-tag value)\nParams: value (string, symbol, or tag)\nReturns: tag\nConverts string, symbol, or existing tag to tag type")
          :code $ quote &runtime-inplementation
        |&compare $ %{} :CodeEntry (:doc "|internal function for native comparison\nSyntax: (&compare a b)\nParams: a (any), b (any)\nReturns: number (-1, 0, or 1)\nPerforms three-way comparison returning -1 (less), 0 (equal), or 1 (greater)")
          :code $ quote &runtime-inplementation
        |&get-os $ %{} :CodeEntry (:doc "|internal function for getting OS information\nSyntax: (&get-os)\nParams: none\nReturns: keyword indicating OS\nReturns current operating system like :linux, :macos, :windows")
          :code $ quote &runtime-inplementation
        |&format-ternary-tree $ %{} :CodeEntry (:doc "|internal function for formatting ternary tree\nSyntax: (&format-ternary-tree tree)\nParams: tree (ternary tree structure)\nReturns: formatted string\nFormats internal ternary tree data structure for debugging")
          :code $ quote &runtime-inplementation
        |&buffer $ %{} :CodeEntry (:doc "|internal function for buffer operations\nSyntax: (&buffer data)\nParams: data (list of numbers or bytes)\nReturns: buffer object\nCreates a binary buffer from list of byte values")
          :code $ quote &runtime-inplementation
        |&hash $ %{} :CodeEntry (:doc "|internal function for hashing\nSyntax: (&hash value)\nParams: value (any)\nReturns: number (hash code)\nComputes hash code for any Calcit value for use in hash tables")
          :code $ quote &runtime-inplementation
        |&extract-code-into-edn $ %{} :CodeEntry (:doc "|internal function for extracting code into EDN\nSyntax: (&extract-code-into-edn code)\nParams: code (quoted code)\nReturns: EDN data structure\nExtracts code structure into EDN format for serialization")
          :code $ quote &runtime-inplementation
        |&data-to-code $ %{} :CodeEntry (:doc "|internal function for converting data to code\nSyntax: (&data-to-code data)\nParams: data (EDN data)\nReturns: quoted code\nConverts EDN data structure back to executable code")
          :code $ quote &runtime-inplementation
        |&cirru-type $ %{} :CodeEntry (:doc "|internal function for getting Cirru type\nSyntax: (&cirru-type cirru-node)\nParams: cirru-node (cirru quote)\nReturns: keyword (:leaf or :list)\nReturns type of Cirru node, either :leaf for atoms or :list for expressions")
          :code $ quote &runtime-inplementation
        |&cirru-nth $ %{} :CodeEntry (:doc "|internal function for Cirru nth operation\nSyntax: (&cirru-nth cirru-list index)\nParams: cirru-list (cirru quote list), index (number)\nReturns: cirru node or nil\nGets nth element from Cirru list node, returns nil if index out of bounds")
          :code $ quote &runtime-inplementation
        |:: $ %{} :CodeEntry (:doc "|internal function for creating tuples\nSyntax: (:: impls & values)\nParams: impls (any), values (any, variable number)\nReturns: tuple with impls and values\nCreates a tuple with specified impls and values")
          :code $ quote &runtime-inplementation
        |%:: $ %{} :CodeEntry (:doc "|internal function for creating enum tuples\nSyntax: (%:: enum tag & values)\nParams: enum (record/enum), tag (tag), values (any, variable number)\nReturns: tuple with enum metadata\nCreates a tagged tuple that carries enum metadata for validation (use &tuple:with-impls to attach impls)")
          :code $ quote &runtime-inplementation
        |&tuple:nth $ %{} :CodeEntry (:doc "|internal function for tuple nth operation\nSyntax: (&tuple:nth tuple index)\nParams: tuple (tuple), index (number)\nReturns: value at index or nil\nGets the value at specified index in tuple, returns nil if out of bounds")
          :code $ quote &runtime-inplementation
        |&tuple:assoc $ %{} :CodeEntry (:doc "|internal function for tuple assoc operation\nSyntax: (&tuple:assoc tuple index value)\nParams: tuple (tuple), index (number), value (any)\nReturns: new tuple with updated value\nReturns new tuple with value at index updated")
          :code $ quote &runtime-inplementation
        |&tuple:count $ %{} :CodeEntry (:doc "|internal function for tuple count operation\nSyntax: (&tuple:count tuple)\nParams: tuple (tuple)\nReturns: number of elements\nReturns the number of elements in the tuple")
          :code $ quote &runtime-inplementation
        |&tuple:impls $ %{} :CodeEntry (:doc "|internal function for getting tuple impls\nSyntax: (&tuple:impls tuple)\nParams: tuple (tuple)\nReturns: impls of the tuple\nReturns the impls/type identifier of the tuple")
          :code $ quote &runtime-inplementation
        |&tuple:params $ %{} :CodeEntry (:doc "|internal function for getting tuple params\nSyntax: (&tuple:params tuple)\nParams: tuple (tuple)\nReturns: list of parameter values\nReturns the parameter values of the tuple as a list")
          :code $ quote &runtime-inplementation
        |&tuple:with-impls $ %{} :CodeEntry (:doc "|internal function for tuple with impls operation\nSyntax: (&tuple:with-impls tuple new-impls)\nParams: tuple (tuple), new-impls (any)\nReturns: new tuple with updated impls\nReturns new tuple with same values but different impls")
          :code $ quote &runtime-inplementation
        |&struct::new $ %{} :CodeEntry (:doc "|internal function for creating struct definitions\nSyntax: (&struct::new name (field type) ...)\nParams: name (tag), field pairs (list)\nReturns: struct definition value\nCreates a struct definition with fields and type annotations")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ &struct::new :Person ([] :name :string) ([] :age :number)
        |&enum::new $ %{} :CodeEntry (:doc "|internal function for creating enum definitions\nSyntax: (&enum::new name (variant type...) ...)\nParams: name (tag), variant entries (list)\nReturns: enum prototype value\nCreates enum variants and payload type annotations")
          :code $ quote &runtime-inplementation
          :examples $ []
            quote $ &enum::new :Result ([] :ok :number) ([] :err :string)
        |&struct:with-impls $ %{} :CodeEntry (:doc "|internal function for struct with impls operation\nSyntax: (&struct:with-impls struct impls)\nParams: struct (struct), impls (record)\nReturns: struct with trait implementations\nAttaches impls info to a struct definition")
          :code $ quote &runtime-inplementation
        |&enum:with-impls $ %{} :CodeEntry (:doc "|internal function for enum with impls operation\nSyntax: (&enum:with-impls enum impls)\nParams: enum (enum), impls (record)\nReturns: enum value with trait implementations\nAttaches impls info to an enum prototype")
          :code $ quote &runtime-inplementation
        |&tuple:enum $ %{} :CodeEntry (:doc "|Get the enum prototype from a tuple\nSyntax: (&tuple:enum tuple)\nParams: tuple (tuple)\nReturns: enum value or nil if not an enum tuple")
          :code $ quote &runtime-inplementation
        |&tuple:enum-has-variant? $ %{} :CodeEntry (:doc "|Check if an enum has a specific variant\nSyntax: (&tuple:enum-has-variant? enum tag)\nParams: enum (enum), tag (tag)\nReturns: bool - true if variant exists")
          :code $ quote &runtime-inplementation
        |&tuple:enum-variant-arity $ %{} :CodeEntry (:doc "|Get the arity of a variant in an enum\nSyntax: (&tuple:enum-variant-arity enum tag)\nParams: enum (enum), tag (tag)\nReturns: number - number of payload fields for the variant")
          :code $ quote &runtime-inplementation
        |&tuple:validate-enum $ %{} :CodeEntry (:doc "|Validate enum tuple tag/arity if enum metadata exists\nSyntax: (&tuple:validate-enum tuple tag)\nParams: tuple (tuple), tag (tag)\nReturns: nil\nRaises error on invalid tag or arity")
          :code $ quote &runtime-inplementation
        |&display-stack $ %{} :CodeEntry (:doc "|internal function for displaying call stack\nSyntax: (&display-stack)\nParams: none\nReturns: string representation of call stack\nReturns formatted string showing current call stack for debugging")
          :code $ quote &runtime-inplementation
        |raise $ %{} :CodeEntry (:doc "|internal function for raising exceptions\nSyntax: (raise message)\nParams: message (string)\nReturns: never returns (throws exception)\nThrows an exception with the given message")
          :code $ quote &runtime-inplementation
        |quit! $ %{} :CodeEntry (:doc "|internal function for quitting program\nSyntax: (quit! exit-code)\nParams: exit-code (number, optional, defaults to 0)\nReturns: never returns (exits program)\nTerminates the program with specified exit code")
          :code $ quote &runtime-inplementation
        |get-env $ %{} :CodeEntry (:doc "|internal function for getting environment variables\nSyntax: (get-env var-name)\nParams: var-name (string)\nReturns: string value or nil\nGets environment variable value, returns nil if not found")
          :code $ quote &runtime-inplementation
        |&get-calcit-backend $ %{} :CodeEntry (:doc "|internal function for getting Calcit backend\nSyntax: (&get-calcit-backend)\nParams: none\nReturns: keyword indicating backend\nReturns current backend like :cr (Calcit Runner) or :js (JavaScript)")
          :code $ quote &runtime-inplementation
        |read-file $ %{} :CodeEntry (:doc "|internal function for reading files\nSyntax: (read-file filepath)\nParams: filepath (string)\nReturns: string content or error\nReads file content as string, throws error if file not found")
          :code $ quote &runtime-inplementation
        |write-file $ %{} :CodeEntry (:doc "|internal function for writing files\nSyntax: (write-file filepath content)\nParams: filepath (string), content (string)\nReturns: nil or error\nWrites string content to file, creates directories if needed")
          :code $ quote &runtime-inplementation
        |is-spreading-mark? $ %{} :CodeEntry (:doc "|internal function for detecting syntax &\nSyntax: (is-spreading-mark? value)\nParams: value (any)\nReturns: boolean\nReturns true if value is the spreading mark symbol &")
          :code $ quote &runtime-inplementation
        |parse-cirru $ %{} :CodeEntry (:doc "|internal function for parsing Cirru\nSyntax: (parse-cirru text)\nParams: text (string)\nReturns: list\nParses Cirru syntax text into nested list structure")
          :code $ quote &runtime-inplementation
        |parse-cirru-list $ %{} :CodeEntry (:doc "|internal function for parsing Cirru list\nSyntax: (parse-cirru-list text)\nParams: text (string)\nReturns: list\nParses Cirru text as a list of expressions")
          :code $ quote &runtime-inplementation
        |format-cirru $ %{} :CodeEntry (:doc "|internal function for formatting Cirru\nSyntax: (format-cirru data)\nParams: data (list)\nReturns: string\nFormats nested list structure into Cirru syntax text")
          :code $ quote &runtime-inplementation
        |format-cirru-one-liner $ %{} :CodeEntry (:doc "|internal function for formatting Cirru as one-liner\nSyntax: (format-cirru-one-liner data)\nParams: data (list)\nReturns: string\nFormats nested list structure into Cirru one-liner syntax text")
          :code $ quote &runtime-inplementation
        |parse-cirru-edn $ %{} :CodeEntry (:doc "|internal function for parsing Cirru EDN\nSyntax: (parse-cirru-edn text)\nParams: text (string)\nReturns: any\nParses Cirru EDN format text into Calcit data structures")
          :code $ quote &runtime-inplementation
        |format-cirru-edn $ %{} :CodeEntry (:doc "|internal function for formatting Cirru EDN\nSyntax: (format-cirru-edn data)\nParams: data (any)\nReturns: string\nFormats Calcit data structures into Cirru EDN format text")
          :code $ quote &runtime-inplementation
        |&cirru-quote:to-list $ %{} :CodeEntry (:doc "|internal function for converting Cirru quote to list\nSyntax: (&cirru-quote:to-list quote)\nParams: quote (cirru-quote)\nReturns: list\nConverts Cirru quote structure to regular list")
          :code $ quote &runtime-inplementation
        |cpu-time $ %{} :CodeEntry (:doc "|internal function for getting CPU time\nSyntax: (cpu-time)\nParams: none\nReturns: number\nReturns current CPU time in milliseconds for performance measurement")
          :code $ quote &runtime-inplementation
        |&= $ %{} :CodeEntry (:doc "|internal function for equality comparison\nSyntax: (&= a b & values)\nParams: a (any), b (any), values (any, variadic)\nReturns: boolean\nReturns true if all values are equal using deep comparison")
          :code $ quote &runtime-inplementation
        |&< $ %{} :CodeEntry (:doc "|internal function for less than comparison\nSyntax: (&< a b & values)\nParams: a (number), b (number), values (number, variadic)\nReturns: boolean\nReturns true if values are in ascending order")
          :code $ quote &runtime-inplementation
        |&> $ %{} :CodeEntry (:doc "|internal function for greater than comparison\nSyntax: (&> a b & values)\nParams: a (number), b (number), values (number, variadic)\nReturns: boolean\nReturns true if values are in descending order")
          :code $ quote &runtime-inplementation
        |not $ %{} :CodeEntry (:doc "|internal function for logical not\nSyntax: (not value)\nParams: value (any)\nReturns: boolean\nReturns true if value is falsy (nil or false), false otherwise")
          :code $ quote &runtime-inplementation
        |identical? $ %{} :CodeEntry (:doc "|internal function for identity comparison\nSyntax: (identical? a b)\nParams: a (any), b (any)\nReturns: boolean\nReturns true if two values are identical (same reference), not just equal")
          :code $ quote &runtime-inplementation
        |&+ $ %{} :CodeEntry (:doc "|internal function for addition\nSyntax: (&+ a b)\nParams: a (number), b (number)\nReturns: number\nAdds two numbers together, supports integers and floats")
          :code $ quote &runtime-inplementation
        |&- $ %{} :CodeEntry (:doc "|internal function for subtraction\nSyntax: (&- a b)\nParams: a (number), b (number)\nReturns: number\nSubtracts second number from first, supports integers and floats")
          :code $ quote &runtime-inplementation
        |&* $ %{} :CodeEntry (:doc "|internal function for multiplication\nSyntax: (&* a b)\nParams: a (number), b (number)\nReturns: number\nMultiplies two numbers together, supports integers and floats")
          :code $ quote &runtime-inplementation
        |&/ $ %{} :CodeEntry (:doc "|internal function for division\nSyntax: (&/ a b)\nParams: a (number), b (number)\nReturns: number\nDivides first number by second, returns float result")
          :code $ quote &runtime-inplementation
        |round $ %{} :CodeEntry (:doc "|internal function for rounding numbers\nSyntax: (round n)\nParams: n (number)\nReturns: number\nRounds number to nearest integer")
          :code $ quote &runtime-inplementation
        |floor $ %{} :CodeEntry (:doc "|internal function for floor operation\nSyntax: (floor n)\nParams: n (number)\nReturns: number\nReturns largest integer less than or equal to n")
          :code $ quote &runtime-inplementation
        |sin $ %{} :CodeEntry (:doc "|internal function for sine\nSyntax: (sin n)\nParams: n (number, radians)\nReturns: number\nReturns sine of angle in radians")
          :code $ quote &runtime-inplementation
        |cos $ %{} :CodeEntry (:doc "|internal function for cosine\nSyntax: (cos n)\nParams: n (number, radians)\nReturns: number\nReturns cosine of angle in radians")
          :code $ quote &runtime-inplementation
        |pow $ %{} :CodeEntry (:doc "|internal function for power operation\nSyntax: (pow base exponent)\nParams: base (number), exponent (number)\nReturns: number\nRaises base to the power of exponent")
          :code $ quote &runtime-inplementation
        |ceil $ %{} :CodeEntry (:doc "|internal function for ceiling operation\nSyntax: (ceil n)\nParams: n (number)\nReturns: number\nReturns smallest integer greater than or equal to n")
          :code $ quote &runtime-inplementation
        |sqrt $ %{} :CodeEntry (:doc "|internal function for square root\nSyntax: (sqrt n)\nParams: n (number)\nReturns: number\nReturns square root of n")
          :code $ quote &runtime-inplementation
        |round? $ %{} :CodeEntry (:doc "|internal function for checking if number is round\nSyntax: (round? n)\nParams: n (number)\nReturns: boolean\nReturns true if number has no fractional part")
          :code $ quote &runtime-inplementation
        |&number:fract $ %{} :CodeEntry (:doc "|internal function for number fractional part\nSyntax: (&number:fract n)\nParams: n (number)\nReturns: number\nReturns fractional part of number (n - floor(n))")
          :code $ quote &runtime-inplementation
        |&number:rem $ %{} :CodeEntry (:doc "|internal function for number remainder\nSyntax: (&number:rem a b)\nParams: a (number), b (number)\nReturns: number\nReturns remainder of a divided by b")
          :code $ quote &runtime-inplementation
        |&number:format $ %{} :CodeEntry (:doc "|internal function for number formatting\nSyntax: (&number:format n)\nParams: n (number)\nReturns: string\nFormats number as string representation")
          :code $ quote &runtime-inplementation
        |&number:display-by $ %{} :CodeEntry (:doc "|internal function for number display by base\nSyntax: (&number:display-by n base)\nParams: n (number), base (integer)\nReturns: string\nDisplays number in specified base (2-36)")
          :code $ quote &runtime-inplementation
        |bit-shl $ %{} :CodeEntry (:doc "|internal function for bit shift left\nSyntax: (bit-shl n shift)\nParams: n (integer), shift (integer)\nReturns: integer\nShifts bits of n left by shift positions")
          :code $ quote &runtime-inplementation
        |bit-shr $ %{} :CodeEntry (:doc "|internal function for bit shift right\nSyntax: (bit-shr n shift)\nParams: n (integer), shift (integer)\nReturns: integer\nShifts bits of n right by shift positions")
          :code $ quote &runtime-inplementation
        |bit-and $ %{} :CodeEntry (:doc "|internal function for bitwise AND\nSyntax: (bit-and a b)\nParams: a (integer), b (integer)\nReturns: integer\nPerforms bitwise AND operation on two integers")
          :code $ quote &runtime-inplementation
        |bit-or $ %{} :CodeEntry (:doc "|internal function for bitwise OR\nSyntax: (bit-or a b)\nParams: a (integer), b (integer)\nReturns: integer\nPerforms bitwise OR operation on two integers")
          :code $ quote &runtime-inplementation
        |bit-xor $ %{} :CodeEntry (:doc "|internal function for bitwise XOR\nSyntax: (bit-xor a b)\nParams: a (integer), b (integer)\nReturns: integer\nPerforms bitwise XOR operation on two integers")
          :code $ quote &runtime-inplementation
        |bit-not $ %{} :CodeEntry (:doc "|internal function for bitwise NOT\nSyntax: (bit-not n)\nParams: n (integer)\nReturns: integer\nPerforms bitwise NOT operation (complement) on integer")
          :code $ quote &runtime-inplementation
        |&str:concat $ %{} :CodeEntry (:doc "|internal function for string concatenation\nSyntax: (&str:concat a b)\nParams: a (string), b (string)\nReturns: string\nConcatenates two strings together")
          :code $ quote &runtime-inplementation
        |trim $ %{} :CodeEntry (:doc "|internal function for trimming strings\nSyntax: (trim s)\nParams: s (string)\nReturns: string\nRemoves whitespace from beginning and end of string")
          :code $ quote &runtime-inplementation
        |&str $ %{} :CodeEntry (:doc "|internal function for string conversion\nSyntax: (&str value)\nParams: value (any)\nReturns: string\nConverts value to string representation")
          :code $ quote &runtime-inplementation
        |turn-string $ %{} :CodeEntry (:doc "|internal function for converting to string\nSyntax: (turn-string value)\nParams: value (any)\nReturns: string\nConverts value to string representation")
          :code $ quote &runtime-inplementation
        |split $ %{} :CodeEntry (:doc "|internal function for splitting strings\nSyntax: (split s delimiter)\nParams: s (string), delimiter (string)\nReturns: list of strings\nSplits string by delimiter into list of substrings")
          :code $ quote &runtime-inplementation
        |split-lines $ %{} :CodeEntry (:doc "|internal function for splitting lines\nSyntax: (split-lines s)\nParams: s (string)\nReturns: list of strings\nSplits string by newlines into list of lines")
          :code $ quote &runtime-inplementation
        |starts-with? $ %{} :CodeEntry (:doc "|internal function for checking string prefix\nSyntax: (starts-with? s prefix)\nParams: s (string), prefix (string)\nReturns: boolean\nReturns true if string starts with prefix")
          :code $ quote &runtime-inplementation
        |ends-with? $ %{} :CodeEntry (:doc "|internal function for checking string suffix\nSyntax: (ends-with? s suffix)\nParams: s (string), suffix (string)\nReturns: boolean\nReturns true if string ends with suffix")
          :code $ quote &runtime-inplementation
        |get-char-code $ %{} :CodeEntry (:doc "|internal function for getting character code\nSyntax: (get-char-code char)\nParams: char (string, single character)\nReturns: number\nReturns Unicode code point of character")
          :code $ quote &runtime-inplementation
        |char-from-code $ %{} :CodeEntry (:doc "|internal function for creating character from code\nSyntax: (char-from-code code)\nParams: code (number)\nReturns: string\nCreates character from Unicode code point")
          :code $ quote &runtime-inplementation
        |to-lispy-string $ %{} :CodeEntry (:doc "|internal function for converting to Lisp string\nSyntax: (to-lispy-string value)\nParams: value (any)\nReturns: string\nConverts value to Lisp-style string representation")
          :code $ quote &runtime-inplementation
        |parse-float $ %{} :CodeEntry (:doc "|internal function for parsing float\nSyntax: (parse-float s)\nParams: s (string)\nReturns: number or nil\nParses string as floating point number, returns nil if invalid")
          :code $ quote &runtime-inplementation
        |blank? $ %{} :CodeEntry (:doc "|internal function for checking if string is blank\nSyntax: (blank? s)\nParams: s (string)\nReturns: boolean\nReturns true if string is empty or contains only whitespace")
          :code $ quote &runtime-inplementation
        |&str:compare $ %{} :CodeEntry (:doc "|internal function for string comparison\nSyntax: (&str:compare a b)\nParams: a (string), b (string)\nReturns: number\nCompares strings lexicographically, returns -1, 0, or 1")
          :code $ quote &runtime-inplementation
        |&str:replace $ %{} :CodeEntry (:doc "|internal function for string replacement\nSyntax: (&str:replace s pattern replacement)\nParams: s (string), pattern (string), replacement (string)\nReturns: string\nReplaces all occurrences of pattern with replacement")
          :code $ quote &runtime-inplementation
        |&str:slice $ %{} :CodeEntry (:doc "|internal function for string slicing\nSyntax: (&str:slice s start end)\nParams: s (string), start (number), end (number)\nReturns: string\nExtracts substring from start to end index")
          :code $ quote &runtime-inplementation
        |&str:find-index $ %{} :CodeEntry (:doc "|internal function for finding string index\nSyntax: (&str:find-index s pattern)\nParams: s (string), pattern (string)\nReturns: number or nil\nFinds first index of pattern in string, returns nil if not found")
          :code $ quote &runtime-inplementation
        |&str:escape $ %{} :CodeEntry (:doc "|internal function for string escaping\nSyntax: (&str:escape s)\nParams: s (string)\nReturns: string\nEscapes special characters in string for safe output")
          :code $ quote &runtime-inplementation
        |&str:count $ %{} :CodeEntry (:doc "|internal function for string character count\nSyntax: (&str:count s)\nParams: s (string)\nReturns: number\nReturns number of characters in string")
          :code $ quote &runtime-inplementation
        |&str:empty? $ %{} :CodeEntry (:doc "|internal function for checking if string is empty\nSyntax: (&str:empty? s)\nParams: s (string)\nReturns: boolean\nReturns true if string has zero length")
          :code $ quote &runtime-inplementation
        |&str:contains? $ %{} :CodeEntry (:doc "|internal function for checking if string contains substring\nSyntax: (&str:contains? s substring)\nParams: s (string), substring (string)\nReturns: boolean\nReturns true if string contains substring")
          :code $ quote &runtime-inplementation
        |&str:includes? $ %{} :CodeEntry (:doc "|internal function for checking if string includes substring\nSyntax: (&str:includes? s substring)\nParams: s (string), substring (string)\nReturns: boolean\nReturns true if string includes substring (alias for contains?)")
          :code $ quote &runtime-inplementation
        |&str:nth $ %{} :CodeEntry (:doc "|internal function for getting nth character\nSyntax: (&str:nth s index)\nParams: s (string), index (number)\nReturns: string or nil\nReturns character at index, nil if index out of bounds")
          :code $ quote &runtime-inplementation
        |&str:first $ %{} :CodeEntry (:doc "|internal function for getting first character\nSyntax: (&str:first s)\nParams: s (string)\nReturns: string or nil\nReturns first character of string, nil if empty")
          :code $ quote &runtime-inplementation
        |&str:rest $ %{} :CodeEntry (:doc "|internal function for getting rest of string\nSyntax: (&str:rest s)\nParams: s (string)\nReturns: string\nReturns string without first character")
          :code $ quote &runtime-inplementation
        |&str:pad-left $ %{} :CodeEntry (:doc "|internal function for left padding string\nSyntax: (&str:pad-left s length pad-char)\nParams: s (string), length (number), pad-char (string)\nReturns: string\nPads string on left to specified length with pad character")
          :code $ quote &runtime-inplementation
        |&str:pad-right $ %{} :CodeEntry (:doc "|internal function for right padding string\nSyntax: (&str:pad-right s length pad-char)\nParams: s (string), length (number), pad-char (string)\nReturns: string\nPads string on right to specified length with pad character")
          :code $ quote &runtime-inplementation
        |[] $ %{} :CodeEntry (:doc "|internal function for creating lists\nSyntax: ([] & elements)\nParams: elements (any, variadic)\nReturns: list\nCreates new list from provided elements")
          :code $ quote &runtime-inplementation
        |append $ %{} :CodeEntry (:doc "|internal function for appending to list\nSyntax: (append list element)\nParams: list (list), element (any)\nReturns: list\nReturns new list with element added at end")
          :code $ quote &runtime-inplementation
        |prepend $ %{} :CodeEntry (:doc "|internal function for prepending to list\nSyntax: (prepend list element)\nParams: list (list), element (any)\nReturns: list\nReturns new list with element added at beginning")
          :code $ quote &runtime-inplementation
        |butlast $ %{} :CodeEntry (:doc "|internal function for getting all but last element\nSyntax: (butlast list)\nParams: list (list)\nReturns: list\nReturns new list without the last element")
          :code $ quote &runtime-inplementation
        |range $ %{} :CodeEntry (:doc "|internal function for creating number ranges\nSyntax: (range start end) or (range end)\nParams: start (number, optional), end (number)\nReturns: list\nCreates list of numbers from start to end (exclusive)")
          :code $ quote &runtime-inplementation
        |sort $ %{} :CodeEntry (:doc "|internal function for sorting lists\nSyntax: (sort list) or (sort list comparator)\nParams: list (list), comparator (function, optional)\nReturns: list\nReturns sorted list using natural order or custom comparator")
          :code $ quote &runtime-inplementation
        |foldl $ %{} :CodeEntry (:doc "|internal function for left fold\nSyntax: (foldl list initial reducer)\nParams: list (list), initial (any), reducer (function)\nReturns: any\nFolds list from left with reducer function and initial value")
          :code $ quote &runtime-inplementation
        |foldl-shortcut $ %{} :CodeEntry (:doc "|internal function for left fold with shortcut\nSyntax: (foldl-shortcut list initial reducer)\nParams: list (list), initial (any), reducer (function)\nReturns: any\nFolds list from left with early termination support")
          :code $ quote &runtime-inplementation
        |foldr-shortcut $ %{} :CodeEntry (:doc "|internal function for right fold with shortcut\nSyntax: (foldr-shortcut list initial reducer)\nParams: list (list), initial (any), reducer (function)\nReturns: any\nFolds list from right with early termination support")
          :code $ quote &runtime-inplementation
        |&list:reverse $ %{} :CodeEntry (:doc "|internal function for reversing lists\nSyntax: (&list:reverse list)\nParams: list (list)\nReturns: list\nReturns new list with elements in reverse order")
          :code $ quote &runtime-inplementation
        |&list:concat $ %{} :CodeEntry (:doc "|internal function for concatenating lists\nSyntax: (&list:concat list1 list2)\nParams: list1 (list), list2 (list)\nReturns: list\nReturns new list with elements from both lists")
          :code $ quote &runtime-inplementation
        |&list:count $ %{} :CodeEntry (:doc "|internal function for counting list elements\nSyntax: (&list:count list)\nParams: list (list)\nReturns: number\nReturns number of elements in list")
          :code $ quote &runtime-inplementation
        |&list:empty? $ %{} :CodeEntry (:doc "|internal function for checking if list is empty\nSyntax: (&list:empty? list)\nParams: list (list)\nReturns: boolean\nReturns true if list has no elements")
          :code $ quote &runtime-inplementation
        |&list:slice $ %{} :CodeEntry (:doc "|internal function for slicing lists\nSyntax: (&list:slice list start end)\nParams: list (list), start (number), end (number)\nReturns: list\nReturns sublist from start to end index")
          :code $ quote &runtime-inplementation
        |&list:assoc-before $ %{} :CodeEntry (:doc "|internal function for associating before element\nSyntax: (&list:assoc-before list target element)\nParams: list (list), target (any), element (any)\nReturns: list\nInserts element before first occurrence of target")
          :code $ quote &runtime-inplementation
        |&list:assoc-after $ %{} :CodeEntry (:doc "|internal function for associating after element\nSyntax: (&list:assoc-after list target element)\nParams: list (list), target (any), element (any)\nReturns: list\nInserts element after first occurrence of target")
          :code $ quote &runtime-inplementation
        |&list:contains? $ %{} :CodeEntry (:doc "|internal function for checking if list contains element\nSyntax: (&list:contains? list element)\nParams: list (list), element (any)\nReturns: boolean\nReturns true if list contains element")
          :code $ quote &runtime-inplementation
        |&list:includes? $ %{} :CodeEntry (:doc "|internal function for checking if list includes element\nSyntax: (&list:includes? list element)\nParams: list (list), element (any)\nReturns: boolean\nReturns true if list includes element (alias for contains?)")
          :code $ quote &runtime-inplementation
        |&list:nth $ %{} :CodeEntry (:doc "|internal function for getting nth list element\nSyntax: (&list:nth list index)\nParams: list (list), index (number)\nReturns: any or nil\nReturns element at index, nil if index out of bounds")
          :code $ quote &runtime-inplementation
        |&list:first $ %{} :CodeEntry (:doc "|internal function for getting first list element\nSyntax: (&list:first list)\nParams: list (list)\nReturns: any or nil\nReturns first element of list, nil if empty")
          :code $ quote &runtime-inplementation
        |&list:rest $ %{} :CodeEntry (:doc "|internal function for getting rest of list\nSyntax: (&list:rest list)\nParams: list (list)\nReturns: list\nReturns list without first element")
          :code $ quote &runtime-inplementation
        |&list:assoc $ %{} :CodeEntry (:doc "|internal function for list association\nSyntax: (&list:assoc list index element)\nParams: list (list), index (number), element (any)\nReturns: list\nReturns new list with element at specified index")
          :code $ quote &runtime-inplementation
        |&list:dissoc $ %{} :CodeEntry (:doc "|internal function for list dissociation\nSyntax: (&list:dissoc list index)\nParams: list (list), index (number)\nReturns: list\nReturns new list without element at specified index")
          :code $ quote &runtime-inplementation
        |&list:to-set $ %{} :CodeEntry (:doc "|internal function for converting list to set\nSyntax: (&list:to-set list)\nParams: list (list)\nReturns: set\nConverts list to set, removing duplicates")
          :code $ quote &runtime-inplementation
        |&list:distinct $ %{} :CodeEntry (:doc "|internal function for getting distinct list elements\nSyntax: (&list:distinct list)\nParams: list (list)\nReturns: list\nReturns new list with duplicate elements removed")
          :code $ quote &runtime-inplementation
        |&{} $ %{} :CodeEntry (:doc "|internal function for creating maps\nSyntax: (&{} & key-value-pairs)\nParams: key-value-pairs (any, variadic)\nReturns: map\nCreates new map from key-value pairs")
          :code $ quote &runtime-inplementation
        |&merge $ %{} :CodeEntry (:doc "|internal function for merging maps\nSyntax: (&merge map1 map2 & maps)\nParams: map1 (map), map2 (map), maps (map, variadic)\nReturns: map\nMerges multiple maps, later values override earlier ones")
          :code $ quote &runtime-inplementation
        |to-pairs $ %{} :CodeEntry (:doc "|internal function for converting to pairs\nSyntax: (to-pairs map)\nParams: map (map)\nReturns: set\nConverts map to an unordered set of [key value] pairs")
          :code $ quote &runtime-inplementation
        |&merge-non-nil $ %{} :CodeEntry (:doc "|internal function for merging non-nil values\nSyntax: (&merge-non-nil map1 map2 & maps)\nParams: map1 (map), map2 (map), maps (map, variadic)\nReturns: map\nMerges maps, skipping nil values")
          :code $ quote &runtime-inplementation
        |&map:get $ %{} :CodeEntry (:doc "|internal function for getting map value\nSyntax: (&map:get map key) or (&map:get map key default)\nParams: map (map), key (any), default (any, optional)\nReturns: any\nGets value for key, returns default if key not found")
          :code $ quote &runtime-inplementation
        |&map:dissoc $ %{} :CodeEntry (:doc "|internal function for map dissociation\nSyntax: (&map:dissoc map key & keys)\nParams: map (map), key (any), keys (any, variadic)\nReturns: map\nReturns new map without specified keys")
          :code $ quote &runtime-inplementation
        |&map:to-list $ %{} :CodeEntry (:doc "|internal function for converting map to list\nSyntax: (&map:to-list map)\nParams: map (map)\nReturns: list\nConverts map to list of [key value] pairs")
          :code $ quote &runtime-inplementation
        |&map:count $ %{} :CodeEntry (:doc "|internal function for counting map entries\nSyntax: (&map:count map)\nParams: map (map)\nReturns: number\nReturns number of key-value pairs in map")
          :code $ quote &runtime-inplementation
        |&map:empty? $ %{} :CodeEntry (:doc "|internal function for checking if map is empty\nSyntax: (&map:empty? map)\nParams: map (map)\nReturns: boolean\nReturns true if map has no entries")
          :code $ quote &runtime-inplementation
        |&map:contains? $ %{} :CodeEntry (:doc "|internal function for checking if map contains key\nSyntax: (&map:contains? map key)\nParams: map (map), key (any)\nReturns: boolean\nReturns true if map contains key")
          :code $ quote &runtime-inplementation
        |&map:includes? $ %{} :CodeEntry (:doc "|internal function for checking if map includes key\nSyntax: (&map:includes? map key)\nParams: map (map), key (any)\nReturns: boolean\nReturns true if map includes key (alias for contains?)")
          :code $ quote &runtime-inplementation
        |&map:destruct $ %{} :CodeEntry (:doc "|internal function for map destructuring\nSyntax: (&map:destruct map pattern)\nParams: map (map), pattern (any)\nReturns: map\nDestructs map according to pattern")
          :code $ quote &runtime-inplementation
        |&map:assoc $ %{} :CodeEntry (:doc "|internal function for map association\nSyntax: (&map:assoc map key value & key-values)\nParams: map (map), key (any), value (any), key-values (any, variadic)\nReturns: map\nReturns new map with key-value associations")
          :code $ quote &runtime-inplementation
        |&map:diff-new $ %{} :CodeEntry (:doc "|internal function for map diff new\nSyntax: (&map:diff-new map1 map2)\nParams: map1 (map), map2 (map)\nReturns: map\nReturns new entries in map2 not in map1")
          :code $ quote &runtime-inplementation
        |&map:diff-keys $ %{} :CodeEntry (:doc "|internal function for map diff keys\nSyntax: (&map:diff-keys map1 map2)\nParams: map1 (map), map2 (map)\nReturns: set\nReturns keys that differ between maps")
          :code $ quote &runtime-inplementation
        |&map:common-keys $ %{} :CodeEntry (:doc "|internal function for map common keys\nSyntax: (&map:common-keys map1 map2)\nParams: map1 (map), map2 (map)\nReturns: set\nReturns keys common to both maps")
          :code $ quote &runtime-inplementation
        |#{} $ %{} :CodeEntry (:doc "|internal function for creating sets\nSyntax: (#{} & elements)\nParams: elements (any, variadic)\nReturns: set\nCreates new set from provided elements")
          :code $ quote &runtime-inplementation
        |&include $ %{} :CodeEntry (:doc "|internal function for including in set\nSyntax: (&include set element)\nParams: set (set), element (any)\nReturns: set\nReturns new set with element included")
          :code $ quote &runtime-inplementation
        |&exclude $ %{} :CodeEntry (:doc "|internal function for excluding from set\nSyntax: (&exclude set element)\nParams: set (set), element (any)\nReturns: set\nReturns new set with element excluded")
          :code $ quote &runtime-inplementation
        |&difference $ %{} :CodeEntry (:doc "|internal function for set difference\nSyntax: (&difference set1 set2)\nParams: set1 (set), set2 (set)\nReturns: set\nReturns elements in set1 but not in set2")
          :code $ quote &runtime-inplementation
        |&union $ %{} :CodeEntry (:doc "|internal function for set union\nSyntax: (&union set1 set2 & sets)\nParams: set1 (set), set2 (set), sets (set, variadic)\nReturns: set\nReturns union of all sets")
          :code $ quote &runtime-inplementation
        |&set:intersection $ %{} :CodeEntry (:doc "|internal function for set intersection\nSyntax: (&set:intersection set1 set2)\nParams: set1 (set), set2 (set)\nReturns: set\nReturns elements common to both sets")
          :code $ quote &runtime-inplementation
        |&set:to-list $ %{} :CodeEntry (:doc "|internal function for converting set to list\nSyntax: (&set:to-list set)\nParams: set (set)\nReturns: list\nConverts set to list of elements")
          :code $ quote &runtime-inplementation
        |&set:count $ %{} :CodeEntry (:doc "|internal function for counting set elements\nSyntax: (&set:count set)\nParams: set (set)\nReturns: number\nReturns number of elements in set")
          :code $ quote &runtime-inplementation
        |&set:empty? $ %{} :CodeEntry (:doc "|internal function for checking if set is empty\nSyntax: (&set:empty? set)\nParams: set (set)\nReturns: boolean\nReturns true if set has no elements")
          :code $ quote &runtime-inplementation
        |&set:includes? $ %{} :CodeEntry (:doc "|internal function for checking if set includes element\nSyntax: (&set:includes? set element)\nParams: set (set), element (any)\nReturns: boolean\nReturns true if set includes element")
          :code $ quote &runtime-inplementation
        |&set:destruct $ %{} :CodeEntry (:doc "|internal function for set destructuring\nSyntax: (&set:destruct set pattern)\nParams: set (set), pattern (any)\nReturns: set\nDestructs set according to pattern")
          :code $ quote &runtime-inplementation
        |&trait::new $ %{} :CodeEntry (:doc "|internal function for creating trait values\nSyntax: (&trait::new name methods)\nParams: name (tag/symbol), methods (list of tags)\nReturns: trait\nCreates a trait definition value")
          :code $ quote &runtime-inplementation
        |atom $ %{} :CodeEntry (:doc "|internal function for creating atoms\nSyntax: (atom value)\nParams: value (any)\nReturns: atom\nCreates new atom with initial value")
          :code $ quote &runtime-inplementation
        |&atom:deref $ %{} :CodeEntry (:doc "|internal function for dereferencing atoms\nSyntax: (&atom:deref atom)\nParams: atom (atom)\nReturns: any\nReturns current value of atom")
          :code $ quote &runtime-inplementation
        |add-watch $ %{} :CodeEntry (:doc "|internal function for adding atom watchers\nSyntax: (add-watch atom key callback)\nParams: atom (atom), key (any), callback (function)\nReturns: atom\nAdds watcher function to atom")
          :code $ quote &runtime-inplementation
        |remove-watch $ %{} :CodeEntry (:doc "|internal function for removing atom watchers\nSyntax: (remove-watch atom key)\nParams: atom (atom), key (any)\nReturns: atom\nRemoves watcher with specified key from atom")
          :code $ quote &runtime-inplementation
        |new-record $ %{} :CodeEntry (:doc "|internal function for creating new records\nSyntax: (new-record name & key-value-pairs)\nParams: name (keyword), key-value-pairs (any, variadic)\nReturns: record\nCreates new record with name and fields")
          :code $ quote &runtime-inplementation
        |new-impl-record $ %{} :CodeEntry (:doc "|internal function for creating new impls records\nSyntax: (new-impl-record impls & key-value-pairs)\nParams: impls (any), key-value-pairs (any, variadic)\nReturns: record\nCreates new record with impls and fields")
          :code $ quote &runtime-inplementation
        |&%{} $ %{} :CodeEntry (:doc "|internal function for native record creation\nSyntax: (&%{} name & key-value-pairs)\nParams: name (keyword), key-value-pairs (any, variadic)\nReturns: record\nCreates native record with name and fields")
          :code $ quote &runtime-inplementation
        |&record:with $ %{} :CodeEntry (:doc "|internal function for record with operation\nSyntax: (&record:with record key value & key-values)\nParams: record (record), key (any), value (any), key-values (any, variadic)\nReturns: record\nReturns new record with updated fields")
          :code $ quote &runtime-inplementation
        |&record:impls $ %{} :CodeEntry (:doc "|internal function for getting record impls\nSyntax: (&record:impls record)\nParams: record (record)\nReturns: any\nReturns impls of record")
          :code $ quote &runtime-inplementation
        |&record:with-impls $ %{} :CodeEntry (:doc "|internal function for record with impls operation\nSyntax: (&record:with-impls record impls)\nParams: record (record), impls (any)\nReturns: record\nReturns new record with specified impls")
          :code $ quote &runtime-inplementation
        |&record:matches? $ %{} :CodeEntry (:doc "|internal function for checking record matches\nSyntax: (&record:matches? record pattern)\nParams: record (record), pattern (any)\nReturns: boolean\nReturns true if record matches pattern")
          :code $ quote &runtime-inplementation
        |&record:from-map $ %{} :CodeEntry (:doc "|internal function for creating record from map\nSyntax: (&record:from-map name map)\nParams: name (keyword), map (map)\nReturns: record\nCreates record from map with specified name")
          :code $ quote &runtime-inplementation
        |&record:get-name $ %{} :CodeEntry (:doc "|internal function for getting record name\nSyntax: (&record:get-name record)\nParams: record (record)\nReturns: keyword\nReturns name of record")
          :code $ quote &runtime-inplementation
        |&record:to-map $ %{} :CodeEntry (:doc "|internal function for converting record to map\nSyntax: (&record:to-map record)\nParams: record (record)\nReturns: map\nConverts record to map")
          :code $ quote &runtime-inplementation
        |&record:count $ %{} :CodeEntry (:doc "|internal function for counting record fields\nSyntax: (&record:count record)\nParams: record (record)\nReturns: number\nReturns number of fields in record")
          :code $ quote &runtime-inplementation
        |&record:contains? $ %{} :CodeEntry (:doc "|internal function for checking if record contains field\nSyntax: (&record:contains? record key)\nParams: record (record), key (any)\nReturns: boolean\nReturns true if record contains field")
          :code $ quote &runtime-inplementation
        |&record:get $ %{} :CodeEntry (:doc "|internal function for getting record field\nSyntax: (&record:get record key) or (&record:get record key default)\nParams: record (record), key (any), default (any, optional)\nReturns: any\nGets field value, returns default if field not found")
          :code $ quote &runtime-inplementation
        |&record:assoc $ %{} :CodeEntry (:doc "|internal function for record field association\nSyntax: (&record:assoc record key value & key-values)\nParams: record (record), key (any), value (any), key-values (any, variadic)\nReturns: record\nReturns new record with field associations")
          :code $ quote &runtime-inplementation
        |&record:extend-as $ %{} :CodeEntry (:doc "|internal function for extending record as new type\nSyntax: (&record:extend-as record new-name)\nParams: record (record), new-name (keyword)\nReturns: record\nExtends record as new type with different name")
          :code $ quote &runtime-inplementation

      :ns $ %{} :CodeEntry (:doc "|built-in function and macros in `calcit.core`")
        :code $ quote
          ns calcit.core $ :require
