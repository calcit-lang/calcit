
{} (:package |calcit)
  :configs $ {} (:init-fn |TODO) (:reload-fn |TODO) (:version |TODO)
    :modules $ []
  :files $ {}
    |calcit.core $ {}
      :defs $ {}
        |%<- $ %{} :CodeEntry
          :code $ quote
            defmacro %<- (& xs)
              quasiquote $ ->%
                ~@ $ reverse xs
          :doc |
        |%{} $ %{} :CodeEntry
          :code $ quote
            defmacro %{} (R & xs)
              &let
                args $ &list:concat & xs
                quasiquote $ &%{} ~R ~@args
          :doc |
        |&<= $ %{} :CodeEntry
          :code $ quote
            defn &<= (a b)
              if (&< a b) true $ &= a b
          :doc |
        |&>= $ %{} :CodeEntry
          :code $ quote
            defn &>= (a b)
              if (&> a b) true $ &= a b
          :doc |
        |&case $ %{} :CodeEntry
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
          :doc |
        |&core-fn-class $ %{} :CodeEntry
          :code $ quote
            defrecord! &core-fn-class
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
          :doc |
        |&core-list-class $ %{} :CodeEntry
          :code $ quote
            defrecord! &core-list-class (:any? any?) (:add append) (:append append) (:assoc &list:assoc) (:assoc-after &list:assoc-after) (:assoc-before &list:assoc-before) (:bind mapcat) (:butlast butlast) (:concat &list:concat) (:contains? &list:contains?) (:includes? &list:includes?) (:count &list:count) (:drop drop) (:each each)
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
          :doc |
        |&core-map-class $ %{} :CodeEntry
          :code $ quote
            defrecord! &core-map-class (:add &map:add-entry) (:assoc &map:assoc) (:common-keys &map:common-keys) (:contains? &map:contains?) (:count &map:count) (:destruct &map:destruct) (:diff-keys &map:diff-keys) (:diff-new &map:diff-new) (:dissoc &map:dissoc)
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
          :doc |
        |&core-nil-class $ %{} :CodeEntry
          :code $ quote
            defrecord! &core-nil-class
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
          :doc |
        |&core-number-class $ %{} :CodeEntry
          :code $ quote
            defrecord! &core-number-class (:ceil ceil)
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
          :doc |
        |&core-set-class $ %{} :CodeEntry
          :code $ quote
            defrecord! &core-set-class (:add include) (:contains? &set:includes?) (:count &set:count) (:destruct &set:destruct) (:difference difference)
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
          :doc |
        |&core-string-class $ %{} :CodeEntry
          :code $ quote
            defrecord! &core-string-class (:blank? blank?) (:count &str:count)
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
          :doc |
        |&doseq $ %{} :CodeEntry
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
          :doc |
        |&field-match-internal $ %{} :CodeEntry
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
          :doc |
        |&init-builtin-classes! $ %{} :CodeEntry
          :code $ quote
            defn &init-builtin-classes! () (; "this function to make sure builtin classes are loaded") (identity &core-number-class) (identity &core-string-class) (identity &core-set-class) (identity &core-list-class) (identity &core-map-class) (identity &core-nil-class) (identity &core-fn-class)
          :doc |
        |&list-match-internal $ %{} :CodeEntry
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
          :doc |
        |&list:filter $ %{} :CodeEntry
          :code $ quote
            defn &list:filter (xs f)
              reduce xs ([])
                defn %&list:filter (acc x)
                  if (f x) (append acc x) acc
          :doc |
        |&list:filter-pair $ %{} :CodeEntry
          :code $ quote
            defn &list:filter-pair (xs f)
              if (list? xs)
                &list:filter xs $ defn %filter-pair (pair)
                  assert "|expected a pair" $ and (list? pair)
                    = 2 $ count pair
                  f (nth pair 0) (nth pair 1)
                raise $ str-spaced "|expected list or map from `filter-pair`, got:" xs
          :doc |
        |&list:find-last $ %{} :CodeEntry
          :code $ quote
            defn &list:find-last (xs f)
              foldr-shortcut xs nil nil $ fn (_acc x)
                if (f x) (:: true x) (:: false nil)
          :doc |
        |&list:find-last-index $ %{} :CodeEntry
          :code $ quote
            defn &list:find-last-index (xs f)
              foldr-shortcut xs
                dec $ count xs
                , nil $ fn (idx x)
                  if (f x) (:: true idx)
                    :: false $ &- 1 idx
          :doc |
        |&list:flatten $ %{} :CodeEntry
          :code $ quote
            defn &list:flatten (xs)
              if (list? xs)
                &list:concat & $ map xs &list:flatten
                [] xs
          :doc |
        |&list:last-index-of $ %{} :CodeEntry
          :code $ quote
            defn &list:last-index-of (xs item)
              foldr-shortcut xs
                dec $ count xs
                , nil $ fn (idx x)
                  if (&= item x) (:: true idx)
                    :: false $ &- 1 idx
          :doc |
        |&list:map $ %{} :CodeEntry
          :code $ quote
            defn &list:map (xs f)
              foldl xs ([])
                defn %&list:map (acc x)
                  append acc $ f x
          :doc |
        |&list:map-pair $ %{} :CodeEntry
          :code $ quote
            defn &list:map-pair (xs f)
              if (list? xs)
                map xs $ defn %map-pair (pair)
                  assert "|expected a pair" $ and (list? pair)
                    = 2 $ count pair
                  f (nth pair 0) (nth pair 1)
                raise $ str-spaced "|expected list or map from `map-pair`, got:" xs
          :doc |
        |&list:max $ %{} :CodeEntry
          :code $ quote
            defn &list:max (xs)
              list-match xs
                () nil
                (x0 xss)
                  reduce xss x0 $ defn %max (acc x) (&max acc x)
          :doc |
        |&list:min $ %{} :CodeEntry
          :code $ quote
            defn &list:min (xs)
              list-match xs
                () nil
                (x0 xss)
                  reduce xss x0 $ defn %min (acc x) (&min acc x)
          :doc |
        |&list:sort-by $ %{} :CodeEntry
          :code $ quote
            defn &list:sort-by (xs f)
              if (tag? f)
                sort xs $ defn %&list:sort-by (a b)
                  &compare (get a f) (get b f)
                sort xs $ defn %&list:sort-by (a b)
                  &compare (f a) (f b)
          :doc |
        |&map:add-entry $ %{} :CodeEntry
          :code $ quote
            defn &map:add-entry (xs pair)
              assert "|&map:add-entry expected value in a pair" $ and (list? pair)
                &= 2 $ count pair
              &map:assoc xs (nth pair 0) (nth pair 1)
          :doc |
        |&map:filter $ %{} :CodeEntry
          :code $ quote
            defn &map:filter (xs f)
              reduce xs (&{})
                defn %&map:filter (acc x)
                  if (f x)
                    &map:assoc acc (nth x 0) (nth x 1)
                    , acc
          :doc |
        |&map:filter-kv $ %{} :CodeEntry
          :code $ quote
            defn &map:filter-kv (xs f)
              reduce xs (&{})
                defn %map:filter-kv (acc x)
                  if
                    f (nth x 0) (nth x 1)
                    &map:assoc acc (nth x 0) (nth x 1)
                    , acc
          :doc |
        |&map:map $ %{} :CodeEntry
          :code $ quote
            defn &map:map (xs f)
              foldl xs ({})
                defn &map:map (acc pair)
                  &let
                    result $ f pair
                    assert "|expected pair returned when mapping hashmap" $ and (list? result)
                      &= 2 $ &list:count result
                    &map:assoc acc (nth result 0) (nth result 1)
          :doc |
        |&map:map-list $ %{} :CodeEntry
          :code $ quote
            defn &map:map-list (xs f)
              if (map? xs)
                foldl xs ([])
                  defn %&map:map-list (acc pair)
                    append acc $ f pair
                raise $ str-spaced "|&map:map-list expected a map, got:" xs
          :doc |
        |&max $ %{} :CodeEntry
          :code $ quote
            defn &max (a b)
              assert "|expects numbers for &max" $ if (number? a) (number? b)
                if (string? a) (string? b) false
              if (&> a b) a b
          :doc |
        |&min $ %{} :CodeEntry
          :code $ quote
            defn &min (a b)
              assert "|expects numbers for &min" $ if (number? a) (number? b)
                if (string? a) (string? b) false
              if (&< a b) a b
          :doc |
        |&record-match-internal $ %{} :CodeEntry
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
          :doc |
        |&set:filter $ %{} :CodeEntry
          :code $ quote
            defn &set:filter (xs f)
              reduce xs (#{})
                defn %&set:filter (acc x)
                  if (f x) (&include acc x) acc
          :doc |
        |&set:max $ %{} :CodeEntry
          :code $ quote
            defn &set:max (xs)
              &let
                pair $ &set:destruct xs
                if (nil? pair) nil $ reduce (nth pair 1) (nth pair 0)
                  defn %max (acc x) (&max acc x)
          :doc |
        |&set:min $ %{} :CodeEntry
          :code $ quote
            defn &set:min (xs)
              &let
                pair $ &set:destruct xs
                if (nil? pair) nil $ reduce (nth pair 1) (nth pair 0)
                  defn %min (acc x) (&min acc x)
          :doc |
        |&str-spaced $ %{} :CodeEntry
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
          :doc |
        |&tag-match-internal $ %{} :CodeEntry
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
                        quasiquote $ if (&= ~t ~k)
                          let
                            ~ $ map-indexed (&list:rest pattern)
                              defn %tag-match (idx x)
                                [] x $ quasiquote
                                  &tuple:nth ~value $ ~ (inc idx)
                            , ~branch
                          &tag-match-internal ~value ~t $ ~@ (&list:rest body)
                      if (&= pattern '_) branch $ raise (str-spaced "|unknown supported pattern:" pair)
          :doc |
        |* $ %{} :CodeEntry
          :code $ quote
            defn * (x & ys) (reduce ys x &*)
          :doc |
        |+ $ %{} :CodeEntry
          :code $ quote
            defn + (x & ys) (reduce ys x &+)
          :doc |
        |- $ %{} :CodeEntry
          :code $ quote
            defn - (x & ys)
              if (&list:empty? ys) (&- 0 x) (reduce ys x &-)
          :doc |
        |-> $ %{} :CodeEntry
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
          :doc |
        |->% $ %{} :CodeEntry
          :code $ quote
            defmacro ->% (base & xs)
              if (&list:empty? xs) base $ let
                  tail $ last xs
                  pairs $ &list:concat
                    [] $ [] '% base
                    map (butlast xs)
                      defn %->% (x) ([] '% x)
                quasiquote $ let ~pairs ~tail
          :doc |
        |->> $ %{} :CodeEntry
          :code $ quote
            defmacro ->> (base & xs)
              if (&list:empty? xs) (quasiquote ~base)
                &let
                  x0 $ &list:first xs
                  if (list? x0)
                    recur (append x0 base) & $ &list:rest xs
                    recur ([] x0 base) & $ &list:rest xs
          :doc |
        |/ $ %{} :CodeEntry
          :code $ quote
            defn / (x & ys)
              if (&list:empty? ys) (&/ 1 x) (reduce ys x &/)
          :doc |
        |/= $ %{} :CodeEntry
          :code $ quote
            defn /= (a b) (not= a b)
          :doc |
        |: $ %{} :CodeEntry
          :code $ quote
            defmacro : (tag & args)
              quasiquote $ ::
                ~ $ turn-tag tag
                ~@ args
          :doc |
        |;nil $ %{} :CodeEntry
          :code $ quote
            defmacro ;nil (& _body) nil
          :doc |
        |< $ %{} :CodeEntry
          :code $ quote
            defn < (x & ys)
              if
                &= 1 $ &list:count ys
                &< x $ &list:first ys
                foldl-compare ys x &<
          :doc |
        |<- $ %{} :CodeEntry
          :code $ quote
            defmacro <- (& xs)
              quasiquote $ ->
                ~@ $ reverse xs
          :doc |
        |<= $ %{} :CodeEntry
          :code $ quote
            defn <= (x & ys)
              if
                &= 1 $ &list:count ys
                &<= x $ &list:first ys
                foldl-compare ys x &<=
          :doc |
        |= $ %{} :CodeEntry
          :code $ quote
            defn = (x & ys)
              if
                &= 1 $ &list:count ys
                &= x $ &list:first ys
                foldl-compare ys x &=
          :doc |
        |> $ %{} :CodeEntry
          :code $ quote
            defn > (x & ys)
              if
                &= 1 $ &list:count ys
                &> x $ &list:first ys
                foldl-compare ys x &>
          :doc |
        |>= $ %{} :CodeEntry
          :code $ quote
            defn >= (x & ys)
              if
                &= 1 $ &list:count ys
                &>= x $ &list:first ys
                foldl-compare ys x &>=
          :doc |
        |[,] $ %{} :CodeEntry
          :code $ quote
            defmacro [,] (& body)
              &let
                xs $ &list:filter body
                  fn (x) (/= x ',)
                quasiquote $ [] ~@xs
          :doc |
        |[][] $ %{} :CodeEntry
          :code $ quote
            defmacro [][] (& xs)
              &let
                items $ map xs
                  fn (ys)
                    quasiquote $ [] ~@ys
                quasiquote $ [] ~@items
          :doc |
        |\ $ %{} :CodeEntry
          :code $ quote
            defmacro \ (& xs)
              quasiquote $ defn %\ (? % %2) ~xs
          :doc |
        |\. $ %{} :CodeEntry
          :code $ quote
            defmacro \. (args-alias & xs)
              &let
                args $ ->% (turn-string args-alias) (split % |.) (map % turn-symbol)
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
          :doc |
        |and $ %{} :CodeEntry
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
          :doc |
        |any? $ %{} :CodeEntry
          :code $ quote
            defn any? (xs f)
              foldl-shortcut xs false false $ defn %any? (acc x)
                if (f x) (:: true true) (:: false acc)
          :doc |
        |apply $ %{} :CodeEntry
          :code $ quote
            defn apply (f args) (f & args)
          :doc |
        |apply-args $ %{} :CodeEntry
          :code $ quote
            defmacro apply-args (args f)
              if
                &= [] $ &list:first args
                quasiquote $ ~f
                  ~@ $ &list:rest args
                quasiquote $ ~f ~@args
          :doc |
        |assert $ %{} :CodeEntry
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
          :doc |
        |assert-detect $ %{} :CodeEntry
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
          :doc |
        |assert= $ %{} :CodeEntry
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
          :doc |
        |assoc $ %{} :CodeEntry
          :code $ quote
            defn assoc (x & args)
              if (nil? x)
                raise $ str-spaced "|assoc does not work on nil for:" args
                if (tuple? x) (&tuple:assoc x & args)
                  if (list? x) (&list:assoc x & args)
                    if (record? x) (&record:assoc x & args) (.assoc x & args)
          :doc |
        |assoc-in $ %{} :CodeEntry
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
          :doc |
        |bool? $ %{} :CodeEntry
          :code $ quote
            defn bool? (x)
              &= (type-of x) :bool
          :doc |
        |buffer? $ %{} :CodeEntry
          :code $ quote
            defn buffer? (x)
              &= (type-of x) :buffer
          :doc |
        |call-w-log $ %{} :CodeEntry
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
          :doc |
        |call-wo-log $ %{} :CodeEntry
          :code $ quote
            defmacro call-wo-log (f & xs)
              quasiquote $ ~f ~@xs
          :doc |
        |case $ %{} :CodeEntry
          :code $ quote
            defmacro case (item & patterns)
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~item) (&case ~v nil ~@patterns)
          :doc |
        |case-default $ %{} :CodeEntry
          :code $ quote
            defmacro case (item default & patterns)
              if (&list:empty? patterns)
                raise $ str-spaced "|Expected patterns for case-default, got empty after:" default
              &let
                v $ gensym |v
                quasiquote $ &let (~v ~item) (&case ~v ~default ~@patterns)
          :doc |
        |concat $ %{} :CodeEntry
          :code $ quote
            defn concat (& args)
              list-match args
                () $ []
                (a0 as) (.concat a0 & as)
          :doc |
        |cond $ %{} :CodeEntry
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
          :doc |
        |conj $ %{} :CodeEntry
          :code $ quote
            defn conj (xs y0 & ys)
              if (empty? ys) (append xs y0)
                recur (append xs y0) & ys
          :doc |
        |contains-in? $ %{} :CodeEntry
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
          :doc |
        |contains-symbol? $ %{} :CodeEntry
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
          :doc |
        |contains? $ %{} :CodeEntry
          :code $ quote
            defn contains? (x k)
              if (nil? x) false $ if (list? x) (&list:contains? x k)
                if (record? x) (&record:contains? x k)
                  if (tuple? x)
                    and (&>= k 0)
                      &< k $ &tuple:count x
                    .contains? x k
          :doc |
        |count $ %{} :CodeEntry
          :code $ quote
            defn count (x)
              if (nil? x) 0 $ if (tuple? x) (&tuple:count x)
                if (list? x) (&list:count x)
                  if (record? x) (&record:count x) (.count x)
          :doc |
        |dec $ %{} :CodeEntry
          :code $ quote
            defn dec (x) (&- x 1)
          :doc |
        |def $ %{} :CodeEntry
          :code $ quote
            defmacro def (name x) x
          :doc |
        |defn-w-log $ %{} :CodeEntry
          :code $ quote
            defmacro defn-w-log (f-name args & body)
              quasiquote $ defn ~f-name ~args
                &let
                  ~f-name $ defn ~f-name ~args ~@body
                  call-w-log ~f-name ~@args
          :doc |
        |defn-wo-log $ %{} :CodeEntry
          :code $ quote
            defmacro defn-wo-log (f-name args & body)
              quasiquote $ defn ~f-name ~args ~@body
          :doc |
        |defrecord $ %{} :CodeEntry
          :code $ quote
            defmacro defrecord (name & xs)
              quasiquote $ new-record
                ~ $ turn-tag name
                , ~@xs
          :doc |
        |defrecord! $ %{} :CodeEntry
          :code $ quote
            defmacro defrecord! (name & pairs)
              quasiquote $ %{}
                new-record
                  ~ $ turn-tag name
                  ~@ $ map pairs &list:first
                , ~@pairs
          :doc |
        |destruct-list $ %{} :CodeEntry
          :code $ quote
            defn destruct-list (xs)
              if (empty? xs) (:: :none)
                :: :some (nth xs 0) (&list:slice xs 1)
          :doc |
        |destruct-map $ %{} :CodeEntry
          :code $ quote
            defn destruct-map (xs)
              &let
                pair $ &map:destruct xs
                if (nil? pair) (:: :none) (:: :some & pair)
          :doc |
        |destruct-set $ %{} :CodeEntry
          :code $ quote
            defn destruct-set (xs)
              &let
                pair $ &set:destruct xs
                if (nil? pair) (:: :none)
                  :: :some (nth pair 0) (nth pair 1)
          :doc |
        |destruct-str $ %{} :CodeEntry
          :code $ quote
            defn destruct-str (s)
              if (&= s |) (:: :none)
                :: :some (nth s 0) (&str:slice s 1)
          :doc |
        |difference $ %{} :CodeEntry
          :code $ quote
            defn difference (base & xs)
              reduce xs base $ fn (acc item) (&difference acc item)
          :doc |
        |dissoc $ %{} :CodeEntry
          :code $ quote
            defn dissoc (x & args)
              if (nil? x) nil $ if (list? x) (&list:dissoc x & args) (.dissoc x & args)
          :doc |
        |dissoc-in $ %{} :CodeEntry
          :code $ quote
            defn dissoc-in (data path)
              list-match path
                () nil
                (p0 ps)
                  if
                    &= 1 $ &list:count path
                    dissoc data p0
                    assoc data p0 $ dissoc-in (get data p0) ps
          :doc |
        |distinct $ %{} :CodeEntry
          :code $ quote
            defn distinct (x) (&list:distinct x)
          :doc |
        |do $ %{} :CodeEntry
          :code $ quote
            defmacro do (& body)
              ; println |body: $ format-to-lisp body
              if (empty? body) (raise "|empty do is not okay")
              quasiquote $ &let () (~@ body)
          :doc |
        |doc-fn $ %{} :CodeEntry
          :code $ quote
            defmacro doc-fn (& _doc) nil
          :doc |
        |drop $ %{} :CodeEntry
          :code $ quote
            defn drop (xs n)
              slice xs n $ &list:count xs
          :doc |
        |each $ %{} :CodeEntry
          :code $ quote
            defn each (xs f)
              foldl xs nil $ defn %each (_acc x) (f x)
          :doc |
        |either $ %{} :CodeEntry
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
          :doc |
        |empty $ %{} :CodeEntry
          :code $ quote
            defn empty (x)
              if (nil? x) nil $ if (list? x) ([]) (.empty x)
          :doc |
        |empty? $ %{} :CodeEntry
          :code $ quote
            defn empty? (x)
              if (nil? x) true $ if (list? x) (&list:empty? x) (.empty? x)
          :doc |
        |ends-with? $ %{} :CodeEntry
          :code $ quote
            defn ends-with? (x y)
              &=
                &- (&str:count x) (&str:count y)
                &str:find-index x y
          :doc |
        |every? $ %{} :CodeEntry
          :code $ quote
            defn every? (xs f)
              foldl-shortcut xs true true $ defn %every? (acc x)
                if (f x) (:: false acc) (:: true false)
          :doc |
        |exclude $ %{} :CodeEntry
          :code $ quote
            defn exclude (base & xs)
              reduce xs base $ fn (acc item) (&exclude acc item)
          :doc |
        |field-match $ %{} :CodeEntry
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
          :doc |
        |filter $ %{} :CodeEntry
          :code $ quote
            defn filter (xs f) (.filter xs f)
          :doc |
        |filter-not $ %{} :CodeEntry
          :code $ quote
            defn filter-not (xs f)
              .filter xs $ defn %filter-not (x)
                not $ f x
          :doc |
        |find $ %{} :CodeEntry
          :code $ quote
            defn find (xs f)
              foldl-shortcut xs 0 nil $ defn %find (_acc x)
                if (f x) (:: true x) (:: false nil)
          :doc |
        |find-index $ %{} :CodeEntry
          :code $ quote
            defn find-index (xs f)
              foldl-shortcut xs 0 nil $ defn %find-index (idx x)
                if (f x) (:: true idx)
                  :: false $ &+ 1 idx
          :doc |
        |first $ %{} :CodeEntry
          :code $ quote
            defn first (x)
              if (nil? x) nil $ if (tuple? x) (&tuple:nth x 0)
                if (list? x) (&list:nth x 0) (.first x)
          :doc |
        |flipped $ %{} :CodeEntry
          :code $ quote
            defmacro flipped (f & args)
              quasiquote $ ~f
                ~@ $ reverse args
          :doc |
        |fn $ %{} :CodeEntry
          :code $ quote
            defmacro fn (args & body)
              quasiquote $ defn f% ~args ~@body
          :doc |
        |fn? $ %{} :CodeEntry
          :code $ quote
            defn fn? (x)
              if
                &= (type-of x) :fn
                , true $ &= (type-of x) :proc
          :doc |
        |foldl' $ %{} :CodeEntry
          :code $ quote
            defn foldl' (xs acc f)
              list-match xs
                () acc
                (x0 xss)
                  recur xss (f acc x0) f
          :doc |
        |foldl-compare $ %{} :CodeEntry
          :code $ quote
            defn foldl-compare (xs acc f)
              if (&list:empty? xs) true $ if
                f acc $ &list:first xs
                recur (&list:rest xs) (&list:first xs) f
                , false
          :doc |
        |frequencies $ %{} :CodeEntry
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
          :doc |
        |get $ %{} :CodeEntry
          :code $ quote
            defn get (base k)
              if (nil? base) nil $ if (string? base) (&str:nth base k)
                if (map? base) (&map:get base k)
                  if (list? base) (&list:nth base k)
                    if (tuple? base) (&tuple:nth base k)
                      if (record? base) (&record:get base k)
                        raise $ str-spaced "|Expected map or list for get, got:" base k
          :doc |
        |get-in $ %{} :CodeEntry
          :code $ quote
            defn get-in (base path)
              if
                not $ list? path
                raise $ str-spaced "|expects path in a list, got:" path
              if (nil? base) nil $ list-match path
                () base
                (y0 ys)
                  recur (get base y0) ys
          :doc |
        |group-by $ %{} :CodeEntry
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
          :doc |
        |identity $ %{} :CodeEntry
          :code $ quote
            defn identity (x) x
          :doc |
        |if-let $ %{} :CodeEntry
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
          :doc |
        |if-not $ %{} :CodeEntry
          :code $ quote
            defmacro if-not (condition true-branch ? false-branch)
              quasiquote $ if ~condition ~false-branch ~true-branch
          :doc |
        |inc $ %{} :CodeEntry
          :code $ quote
            defn inc (x) (&+ x 1)
          :doc |
        |include $ %{} :CodeEntry
          :code $ quote
            defn include (base & xs)
              reduce xs base $ fn (acc item) (&include acc item)
          :doc |
        |includes? $ %{} :CodeEntry
          :code $ quote
            defn includes? (x k)
              if (nil? x) false $ if (list? x) (&list:includes? x k) (.includes? x k)
          :doc |
        |index-of $ %{} :CodeEntry
          :code $ quote
            defn index-of (xs item)
              foldl-shortcut xs 0 nil $ defn %index-of (idx x)
                if (&= item x) (:: true idx)
                  :: false $ &+ 1 idx
          :doc |
        |interleave $ %{} :CodeEntry
          :code $ quote
            defn interleave (xs0 ys0)
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
          :doc |
        |intersection $ %{} :CodeEntry
          :code $ quote
            defn intersection (base & xs)
              reduce xs base $ fn (acc item) (&set:intersection acc item)
          :doc |
        |invoke $ %{} :CodeEntry
          :code $ quote
            defn invoke (pair name & params)
              if
                not $ and (list? pair)
                  = 2 $ &list:count pair
                  record? $ &list:first pair
                raise $ str-spaced "|method! applies on a pair, leading by record, got:" pair
              if
                not $ or (string? name) (tag? name) (symbol? name)
                raise $ str-spaced "|method by string or tag, got:" name
              let
                  proto $ &tuple:nth pair 0
                  f $ &record:get proto name
                if
                  not $ fn? f
                  raise $ str-spaced "|expected function, got:" f
                f pair & params
          :doc |
        |join $ %{} :CodeEntry
          :code $ quote
            defn join (xs0 sep)
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
          :doc |
        |join-str $ %{} :CodeEntry
          :code $ quote
            defn join-str (xs0 sep)
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
          :doc |
        |js-object $ %{} :CodeEntry
          :code $ quote
            defmacro js-object (& xs)
              &let
                ys $ &list:concat & xs
                quasiquote $ &js-object ~@ys
          :doc |
        |keys $ %{} :CodeEntry
          :code $ quote
            defn keys (x)
              map (to-pairs x) &list:first
          :doc |
        |keys-non-nil $ %{} :CodeEntry
          :code $ quote
            defn keys-non-nil (x)
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
          :doc |
        |last $ %{} :CodeEntry
          :code $ quote
            defn last (xs)
              if (empty? xs) nil $ nth xs
                &- (count xs) 1
          :doc |
        |let $ %{} :CodeEntry
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
          :doc |
        |let-destruct $ %{} :CodeEntry
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
          :doc |
        |let-sugar $ %{} :CodeEntry
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
          :doc |
        |let[] $ %{} :CodeEntry
          :code $ quote
            defmacro let[] (vars data & body)
              if
                not $ and (list? vars) (every? vars symbol?)
                raise $ str-spaced "|expects a list of definitions, got:" vars
              let
                  variable? $ symbol? data
                  v $ if variable? data (gensym |v)
                  defs $ apply-args
                    [] ([]) vars 0
                    defn let[]% (acc xs idx)
                      if (&list:empty? xs) acc $ &let ()
                        if
                          not $ symbol? (&list:first xs)
                          raise $ &str:concat "|Expected symbol for vars: " (&list:first xs)
                        if
                          &= (&list:first xs) '&
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
          :doc |
        |let{} $ %{} :CodeEntry
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
          :doc |
        |list-match $ %{} :CodeEntry
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
          :doc |
        |list? $ %{} :CodeEntry
          :code $ quote
            defn list? (x)
              &= (type-of x) :list
          :doc |
        |loop $ %{} :CodeEntry
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
          :doc |
        |macro? $ %{} :CodeEntry
          :code $ quote
            defn macro? (x)
              &= (type-of x) :macro
          :doc |
        |map $ %{} :CodeEntry
          :code $ quote
            defn map (xs f)
              if (list? xs) (&list:map xs f)
                if (set? xs)
                  foldl xs (#{})
                    defn %map (acc x)
                      include acc $ f x
                  if (map? xs) (&map:map xs f)
                    raise $ str-spaced "|expected list or set for map function, got:" xs
          :doc |
        |map-indexed $ %{} :CodeEntry
          :code $ quote
            defn map-indexed (xs f)
              foldl xs ([])
                defn %map-indexed (acc x)
                  append acc $ f (count acc) x
          :doc |
        |map-kv $ %{} :CodeEntry
          :code $ quote
            defn map-kv (xs f)
              assert "|expects a map" $ map? xs
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
          :doc |
        |map? $ %{} :CodeEntry
          :code $ quote
            defn map? (x)
              &= (type-of x) :map
          :doc |
        |mapcat $ %{} :CodeEntry
          :code $ quote
            defn mapcat (xs f)
              &list:concat & $ map xs f
          :doc |
        |max $ %{} :CodeEntry
          :code $ quote
            defn max (xs) (.max xs)
          :doc |
        |merge $ %{} :CodeEntry
          :code $ quote
            defn merge (x0 & xs) (reduce xs x0 &merge)
          :doc |
        |merge-non-nil $ %{} :CodeEntry
          :code $ quote
            defn merge-non-nil (x0 & xs) (reduce xs x0 &merge-non-nil)
          :doc |
        |min $ %{} :CodeEntry
          :code $ quote
            defn min (xs) (.min xs)
          :doc |
        |negate $ %{} :CodeEntry
          :code $ quote
            defn negate (x) (&- 0 x)
          :doc |
        |nil? $ %{} :CodeEntry
          :code $ quote
            defn nil? (x)
              &= (type-of x) :nil
          :doc |
        |not= $ %{} :CodeEntry
          :code $ quote
            defn not= (x y)
              not $ &= x y
          :doc |
        |noted $ %{} :CodeEntry
          :code $ quote
            defmacro noted (_doc v) v
          :doc |
        |nth $ %{} :CodeEntry
          :code $ quote
            defn nth (x i)
              if (tuple? x) (&tuple:nth x i)
                if (list? x) (&list:nth x i) (.nth x i)
          :doc |
        |number? $ %{} :CodeEntry
          :code $ quote
            defn number? (x)
              &= (type-of x) :number
          :doc |
        |optionally $ %{} :CodeEntry
          :code $ quote
            defn optionally (s)
              if (nil? s) (:: :none) (:: :some s)
          :doc |
        |or $ %{} :CodeEntry
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
          :doc |
        |pairs-map $ %{} :CodeEntry
          :code $ quote
            defn pairs-map (xs)
              reduce xs ({})
                defn %pairs-map (acc pair)
                  assert "|expects pair for pairs-map" $ if (list? pair)
                    &= 2 $ &list:count pair
                    , false
                  &map:assoc acc (&list:first pair) (last pair)
          :doc |
        |print-values $ %{} :CodeEntry
          :code $ quote
            defn print-values (& args)
              println & $ &list:map args pr-str
          :doc |
        |range-bothway $ %{} :CodeEntry
          :code $ quote
            defn range-bothway (x ? y)
              if (some? y)
                range
                  inc $ &- (&+ x x) y
                  , y
                range
                  inc $ negate x
                  , x
          :doc |
        |record-match $ %{} :CodeEntry
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
          :doc |
        |record? $ %{} :CodeEntry
          :code $ quote
            defn record? (x)
              &= (type-of x) :record
          :doc |
        |reduce $ %{} :CodeEntry
          :code $ quote
            defn reduce (xs x0 f) (foldl xs x0 f)
          :doc |
        |ref? $ %{} :CodeEntry
          :code $ quote
            defn ref? (x)
              &= (type-of x) :ref
          :doc |
        |repeat $ %{} :CodeEntry
          :code $ quote
            defn repeat (x n0)
              apply-args
                  []
                  , n0
                defn %repeat (acc n)
                  if (&<= n 0) acc $ recur (append acc x) (&- n 1)
          :doc |
        |rest $ %{} :CodeEntry
          :code $ quote
            defn rest (x)
              if (nil? x) nil $ if (list? x) (&list:rest x) (.rest x)
          :doc |
        |reverse $ %{} :CodeEntry
          :code $ quote
            defn reverse (x) (&list:reverse x)
          :doc |
        |section-by $ %{} :CodeEntry
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
          :doc |
        |select-keys $ %{} :CodeEntry
          :code $ quote
            defn select-keys (m xs)
              assert "|expected map for selecting" $ map? m
              foldl xs (&{})
                defn %select-keys (acc k)
                  &map:assoc acc k $ &map:get m k
          :doc |
        |set? $ %{} :CodeEntry
          :code $ quote
            defn set? (x)
              &= (type-of x) :set
          :doc |
        |slice $ %{} :CodeEntry
          :code $ quote
            defn slice (xs n ? m)
              if (nil? xs) nil $ .slice xs n m
          :doc |
        |some-in? $ %{} :CodeEntry
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
          :doc |
        |some? $ %{} :CodeEntry
          :code $ quote
            defn some? (x)
              not $ nil? x
          :doc |
        |starts-with? $ %{} :CodeEntry
          :code $ quote
            defn starts-with? (x y)
              &= 0 $ &str:find-index x y
          :doc |
        |str $ %{} :CodeEntry
          :code $ quote
            defn str (x0 & xs)
              if (&list:empty? xs) (&str x0)
                &str:concat x0 $ str & xs
          :doc |
        |str-spaced $ %{} :CodeEntry
          :code $ quote
            defn str-spaced (& xs) (&str-spaced true & xs)
          :doc |
        |string? $ %{} :CodeEntry
          :code $ quote
            defn string? (x)
              &= (type-of x) :string
          :doc |
        |strip-prefix $ %{} :CodeEntry
          :code $ quote
            defn strip-prefix (s piece)
              if (starts-with? s piece)
                &str:slice s $ &str:count piece
                , s
          :doc |
        |strip-suffix $ %{} :CodeEntry
          :code $ quote
            defn strip-suffix (s piece)
              if (ends-with? s piece)
                &str:slice s 0 $ &- (&str:count s) (&str:count piece)
                , s
          :doc |
        |swap! $ %{} :CodeEntry
          :code $ quote
            defmacro swap! (a f & args)
              quasiquote $ reset! ~a
                ~f (deref ~a) ~@args
          :doc |
        |symbol? $ %{} :CodeEntry
          :code $ quote
            defn symbol? (x)
              &= (type-of x) :symbol
          :doc |
        |tag-match $ %{} :CodeEntry
          :code $ quote
            defmacro tag-match (value & body)
              if (&list:empty? body)
                quasiquote $ eprintln "|[Error] tag-match expected some patterns and matches" ~value
                &let
                  t# $ gensym |tag
                  if (list? value)
                    &let
                      v# $ gensym |v
                      quasiquote $ &let (~v# ~value)
                        if
                          not $ tuple? ~v#
                          raise $ str "|tag-match expected tuple, got" ~v#
                        &let
                          ~t# $ &tuple:nth ~value 0
                          &tag-match-internal ~v# ~t# $ ~@ body
                    quasiquote $ &let ()
                      if
                        not $ tuple? ~value
                        raise $ str "|tag-match expected tuple, got" ~value
                      &let
                        ~t# $ &tuple:nth ~value 0
                        &tag-match-internal ~value ~t# $ ~@ body
          :doc |
        |tag? $ %{} :CodeEntry
          :code $ quote
            defn tag? (x)
              &= (type-of x) :tag
          :doc |
        |tagging-edn $ %{} :CodeEntry
          :code $ quote
            defn tagging-edn (data)
              if (list? data) (map data tagging-edn)
                if (map? data)
                  map-kv data $ defn %tagging (k v)
                    []
                      if (string? k) (turn-tag k) k
                      tagging-edn v
                  , data
          :doc |
        |take $ %{} :CodeEntry
          :code $ quote
            defn take (xs n)
              if
                >= n $ &list:count xs
                , xs $ slice xs 0 n
          :doc |
        |take-last $ %{} :CodeEntry
          :code $ quote
            defn take-last (xs n)
              if
                >= n $ &list:count xs
                , xs $ slice xs
                  - (&list:count xs) n
                  &list:count xs
          :doc |
        |tuple? $ %{} :CodeEntry
          :code $ quote
            defn tuple? (x)
              &= (type-of x) :tuple
          :doc |
        |turn-str $ %{} :CodeEntry
          :code $ quote
            defn turn-str (x) (turn-string x)
          :doc |
        |union $ %{} :CodeEntry
          :code $ quote
            defn union (base & xs)
              reduce xs base $ fn (acc item) (&union acc item)
          :doc |
        |unselect-keys $ %{} :CodeEntry
          :code $ quote
            defn unselect-keys (m xs)
              assert "|expected map for unselecting" $ map? m
              foldl xs m $ defn %unselect-keys (acc k) (&map:dissoc acc k)
          :doc |
        |update $ %{} :CodeEntry
          :code $ quote
            defn update (x k f)
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
                      raise $ &str:concat "|Cannot update key on item: " (pr-str x)
          :doc |
        |update-in $ %{} :CodeEntry
          :code $ quote
            defn update-in (data path f)
              list-match path
                () $ f data
                (p0 ps)
                  assoc data p0 $ update-in (get data p0) ps f
          :doc |
        |vals $ %{} :CodeEntry
          :code $ quote
            defn vals (x)
              map (to-pairs x) last
          :doc |
        |w-js-log $ %{} :CodeEntry
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
          :doc |
        |w-log $ %{} :CodeEntry
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
          :doc |
        |when $ %{} :CodeEntry
          :code $ quote
            defmacro when (condition & body)
              if
                &= 1 $ &list:count body
                quasiquote $ if ~condition
                  ~ $ nth body 0
                quasiquote $ if ~condition
                  &let () ~@body
          :doc |
        |when-let $ %{} :CodeEntry
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
          :doc |
        |when-not $ %{} :CodeEntry
          :code $ quote
            defmacro when-not (condition & body)
              if
                &= 1 $ &list:count body
                quasiquote $ if (not ~condition)
                  ~ $ nth body 0
                quasiquote $ if (not ~condition)
                  &let () ~@body
          :doc |
        |with-cpu-time $ %{} :CodeEntry
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
          :doc |
        |wo-js-log $ %{} :CodeEntry
          :code $ quote
            defmacro w-js-log (x) x
          :doc |
        |wo-log $ %{} :CodeEntry
          :code $ quote
            defmacro wo-log (x) x
          :doc |
        |zipmap $ %{} :CodeEntry
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
          :doc |
        |{,} $ %{} :CodeEntry
          :code $ quote
            defmacro {,} (& body)
              &let
                xs $ &list:filter body
                  defn %{,} (x) (not= x ',)
                quasiquote $ pairs-map
                  section-by ([] ~@xs) 2
          :doc |
        |{} $ %{} :CodeEntry
          :code $ quote
            defmacro {} (& xs)
              &let
                ys $ &list:concat & xs
                quasiquote $ &{} ~@ys
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns calcit.core $ :require
        :doc |
