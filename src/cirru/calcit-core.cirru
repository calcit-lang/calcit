
{} (:package |calcit)
  :configs $ {}
    :init-fn |TODO
    :reload-fn |TODO
    :version |TODO
    :modules $ []
  :files $ {}
    |calcit.core $ {}
      :ns $ quote
        ns calcit.core $ :require
      :defs $ {}
        |if-not $ quote
          defmacro if-not (condition true-branch ? false-branch)
            quasiquote $ if ~condition ~false-branch ~true-branch

        |not= $ quote
          defn not= (x y) $ not $ &= x y

        |&<= $ quote
          defn &<= (a b)
            if (&< a b) true (&= a b)

        |&>= $ quote
          defn &>= (a b)
            if (&> a b) true (&= a b)

        |last $ quote
          defn last (xs)
            if (empty? xs) nil
              &get xs
                &- (count xs) 1

        |when $ quote
          defmacro when (condition & body)
            quote-replace $ if ~condition (&let nil ~@body)

        |when-not $ quote
          defmacro when-not (condition & body)
            quote-replace $ if (not ~condition) (&let nil ~@body)

        |+ $ quote
          defn + (x & ys) $ reduce ys x &+

        |- $ quote
          defn - (x & ys)
            if (empty? ys)
              &- 0 x
              reduce ys x &-

        |* $ quote
          defn * (x & ys) $ reduce ys x &*

        |/ $ quote
          defn / (x & ys)
            if (empty? ys)
              &/ 1 x
              reduce ys x &/

        |foldl-compare $ quote
          defn foldl-compare (xs acc f)
            if (empty? xs) true
              if (f acc (first xs))
                recur (rest xs) (first xs) f
                , false

        |foldl' $ quote
          defn foldl' (xs acc f)
            if (empty? xs) acc
              recur (rest xs) (f acc (first xs)) f

        |< $ quote
          defn < (x & ys)
            if
              &= 1 (count ys)
              &< x (first ys)
              foldl-compare ys x &<

        |> $ quote
          defn > (x & ys)
            if
              &= 1 (count ys)
              &> x (first ys)
              foldl-compare ys x &>

        |= $ quote
          defn = (x & ys)
            if
              &= 1 (count ys)
              &= x (first ys)
              foldl-compare ys x &=

        |>= $ quote
          defn >= (x & ys)
            if
              &= 1 (count ys)
              &>= x (first ys)
              foldl-compare ys x &>=

        |<= $ quote
          defn <= (x & ys)
            if
              &= 1 (count ys)
              &<= x (first ys)
              foldl-compare ys x &<=

        |apply $ quote
          defn apply (f args) $ f & args

        |apply-args $ quote
          defmacro apply-args (args f)
            if (&= '[] (first args))
              quasiquote
                ~f (~@ (rest args))
              quasiquote
                ~f ~@args

        |list? $ quote
          defn list? (x) $ &= (type-of x) :list

        |map? $ quote
          defn map? (x) $ &= (type-of x) :map

        |number? $ quote
          defn number? (x) $ &= (type-of x) :number

        |string? $ quote
          defn string? (x) $ &= (type-of x) :string

        |symbol? $ quote
          defn symbol? (x) $ &= (type-of x) :symbol

        |keyword? $ quote
          defn keyword? (x) $ &= (type-of x) :keyword

        |bool? $ quote
          defn bool? (x) $ &= (type-of x) :bool

        |nil? $ quote
          defn nil? (x) $ &= (type-of x) :nil

        |ref? $ quote
          defn ref? (x) $ &= (type-of x) :ref

        |tuple? $ quote
          defn tuple? (x) $ &= (type-of x) :tuple

        |record? $ quote
          defn record? (x) $ &= (type-of x) :record

        |macro? $ quote
          defn macro? (x) $ &= (type-of x) :macro

        |set? $ quote
          defn set? (x) $ &= (type-of x) :set

        |fn? $ quote
          defn fn? (x)
            if
              &= (type-of x) :fn
              , true
              &= (type-of x) :proc

        |each $ quote
          defn each (xs f)
            foldl xs nil $ fn (_acc x)
              f x

        |map $ quote
          defn map (xs f)
            cond
              (list? xs)
                foldl xs ([])
                  fn (acc x) $ append acc (f x)
              (set? xs)
                foldl xs (#{})
                  fn (acc x) $ include acc (f x)
              (map? xs)
                foldl xs ({})
                  fn (acc pair) $ let[] (k v) pair
                    &let
                      result (f $ [] k v)
                      assert "|expected pair returned when mapping hashmap"
                        and (list? result) (&= 2 (count result))
                      let[] (k2 v2) result
                        assoc acc k2 v2
              true
                &let nil
                  echo "|value:" xs
                  raise "|expects list or set for map function"

        |take $ quote
          defn take (xs n)
            if (= n (count xs)) xs
              slice xs 0 n

        |drop $ quote
          defn drop (xs n)
            slice xs n (count xs)

        |str $ quote
          defmacro str (x0 & xs)
            if (empty? xs)
              quote-replace $ &str ~x0
              quote-replace $ &str-concat ~x0 $ str ~@xs

        |include $ quote
          defn include (base & xs)
            reduce xs base
              fn (acc item) $ &include acc item

        |exclude $ quote
          defn exclude (base & xs)
            reduce xs base
              fn (acc item) $ &exclude acc item

        |difference $ quote
          defn difference (base & xs)
            reduce xs base
              fn (acc item) $ &difference acc item

        |union $ quote
          defn union (base & xs)
            reduce xs base
              fn (acc item) $ &union acc item

        |intersection $ quote
          defn intersection (base & xs)
            reduce xs base
              fn (acc item) $ &intersection acc item

        |index-of $ quote
          defn index-of (xs item)
            foldl-shortcut xs 0 nil $ fn (idx x)
              if (&= item x)
                [] true idx
                [] false (&+ 1 idx)

        |find-index $ quote
          defn find-index (xs f)
            foldl-shortcut xs 0 nil $ fn (idx x)
              if (f x)
                [] true idx
                [] false (&+ 1 idx)

        |find $ quote
          defn find (xs f)
            foldl-shortcut xs 0 nil $ fn (_acc x)
              if (f x)
                [] true x
                [] false nil

        |-> $ quote
          defmacro -> (base & xs)
            if (empty? xs)
              quote-replace ~base
              &let
                x0 (first xs)
                if (list? x0)
                  recur
                    concat ([] (first x0) base) (rest x0)
                    , & (rest xs)
                  recur ([] x0 base) & (rest xs)

        |->> $ quote
          defmacro ->> (base & xs)
            if (empty? xs)
              quote-replace ~base
              &let
                x0 (first xs)
                if (list? x0)
                  recur (append x0 base) & (rest xs)
                  recur ([] x0 base) & (rest xs)

        |->% $ quote
          defmacro ->% (base & xs)
            if (empty? xs) base
              let
                  tail $ last xs
                  pairs $ concat
                    [] $ [] '% base
                    map
                      butlast xs
                      fn (x) ([] '% x)
                quote-replace
                  let ~pairs ~tail

        |cond $ quote
          defmacro cond (pair & else)
            assert "|expects a pair"
              if (list? pair) (&= 2 (count pair)) false
            &let
              expr $ nth pair 0
              &let
                branch $ nth pair 1
                quote-replace
                  if ~expr ~branch
                    ~ $ if (empty? else) nil
                      quote-replace
                        cond
                          ~ $ nth else 0
                          ~@ $ rest else

        |key-match $ quote
          defmacro key-match (value & body)
            if (empty? body)
              quasiquote
                &let nil
                  echo "|[warn] key-match found no matched case, missing `_` case?" ~value
              &let
                pair (first body)
                assert "|key-match expected pairs"
                  and (list? pair) (&= 2 (count pair))
                let[] (pattern branch) pair
                  if (&= pattern '_) branch
                    &let nil
                      assert "|pattern in a list" (list? pattern)
                      &let
                        k (first pattern)
                        &let (v# (gensym 'v))
                          quasiquote
                            &let (~v# ~value)
                              if (&= (first ~v#) ~k)
                                let
                                  ~ $ map-indexed (rest pattern) $ fn (idx x)
                                    [] x $ quasiquote
                                      nth ~v# (~ (inc idx))
                                  , ~branch
                                key-match ~value (~@ (rest body))

        |&case $ quote
          defmacro &case (item default pattern & others)
            assert "|`case` expects pattern in a pair"
              if (list? pattern) (&= 2 (count pattern)) false
            let
                x $ first pattern
                branch $ last pattern
              quote-replace
                if (&= ~item ~x) ~branch
                  ~ $ if (empty? others) default
                    quote-replace
                      &case ~item ~default ~@others

        |case $ quote
          defmacro case (item & patterns)
            &let
              v (gensym |v)
              quote-replace
                &let
                  ~v ~item
                  &case ~v nil ~@patterns

        |case-default $ quote
          defmacro case (item default & patterns)
            if (empty? patterns)
              raise "|Expected patterns for case-default, got empty"
            &let
              v (gensym |v)
              quote-replace
                &let (~v ~item)
                  &case ~v ~default ~@patterns

        |get $ quote
          defn get (base k)
            cond
              (nil? base) nil
              (string? base) (nth base k)
              (map? base) (&get base k)
              (list? base) (nth base k)
              (record? base) (&get base k)
              true $ &let nil
                echo "|Value:" base k
                raise "|Expected map or list for get"

        |get-in $ quote
          defn get-in (base path)
            assert "|expects path in a list" (list? path)
            cond
              (nil? base) nil
              (empty? path) base
              true
                recur
                  get base (first path)
                  rest path

        |&max $ quote
          defn &max (a b)
            assert "|expects numbers for &max" $ if (number? a) (number? b) false
            if (&> a b) a b

        |&min $ quote
          defn &min (a b)
            assert "|expects numbers for &min" $ if (number? a) (number? b) false
            if (&< a b) a b

        |max $ quote
          defn max (xs)
            if (empty? xs) nil
              reduce (rest xs) (first xs)
                fn (acc x) (&max acc x)

        |min $ quote
          defn min (xs)
            if (empty? xs) nil
              reduce (rest xs) (first xs)
                fn (acc x) (&min acc x)

        |every? $ quote
          defn every? (xs f)
            foldl-shortcut xs nil true $ fn (_acc x)
              if (f x)
                [] false nil
                [] true false

        |any? $ quote
          defn any? (xs f)
            foldl-shortcut xs nil false $ fn (_acc x)
              if (f x)
                [] true true
                [] false nil

        |mapcat $ quote
          defn mapcat (xs f)
            concat & $ map xs f

        |merge $ quote
          defn merge (x0 & xs)
            reduce xs x0 &merge

        |merge-non-nil $ quote
          defn merge-non-nil (x0 & xs)
            reduce xs x0 &merge-non-nil

        |identity $ quote
          defn identity (x) x

        |map-indexed $ quote
          defn map-indexed (xs f)
            foldl xs ([]) $ fn (acc x)
              append acc $ f (count acc) x

        |filter $ quote
          defn filter (xs f)
            foldl xs (empty xs)
              fn (acc x)
                if (f x) (coll-append acc x) acc

        |filter-not $ quote
          defn filter-not (xs f)
            reduce xs (empty xs)
              fn (acc x)
                if-not (f x) (coll-append acc x) acc

        |coll-append $ quote
          defn coll-append (xs a)
            if (list? xs) (append xs a)
              if (set? xs) (&include xs a)
                if (map? xs)
                  &let nil
                    assert "|coll-append to map expected a pair" $ and (list? a)
                      &= 2 (count a)
                    let[] (k v) a
                      assoc xs k v
                  raise "|coll-append expected a collection"

        |empty $ quote
          defn empty (x)
            if (list? x) ([])
              if (map? x) (&{})
                if (set? x) (#{})
                  if (string? x) |
                    if (nil? x) nil
                      raise $ &str-concat "|empty does not work on this type: " (&str x)

        |pairs-map $ quote
          defn pairs-map (xs)
            reduce xs ({})
              fn (acc pair)
                assert "|expects pair for pairs-map"
                  if (list? pair)
                    &= 2 (count pair)
                    , false
                assoc acc (first pair) (last pair)

        |some? $ quote
          defn some? (x) $ not $ nil? x

        |some-in? $ quote
          defn some-in? (x path)
            if (nil? x) false
              if (empty? path) true
                &let (k $ first path)
                  if (map? x)
                    if (contains? x k)
                      recur (get x k) (rest path)
                      , false
                    if (list? x)
                      if (number? k)
                        recur (get x k) (rest path)
                        , false
                      raise $ &str-concat "|Unknown structure for some-in? detection" x


        |zipmap $ quote
          defn zipmap (xs0 ys0)
            apply-args
              ({}) xs0 ys0
              fn (acc xs ys)
                if
                  if (empty? xs) true (empty? ys)
                  , acc
                  recur
                    assoc acc (first xs) (first ys)
                    rest xs
                    rest ys

        |rand-nth $ quote
          defn rand-nth (xs)
            if (empty? xs) nil
              get xs $ rand-int $ &- (count xs) 1

        |contains-symbol? $ quote
          defn contains-symbol? (xs y)
            if (list? xs)
              apply-args (xs)
                fn (body)
                  if (empty? body) false
                    if
                      contains-symbol? (first body) y
                      , true
                      recur (rest body)
              &= xs y

        |\ $ quote
          defmacro \ (& xs)
            quote-replace $ fn (? % %2) ~xs

        |\. $ quote
          defmacro \. (args-alias & xs)
            &let
              args $ ->% (turn-string args-alias) (split % |.) (map % turn-symbol)
              &let
                inner-body $ if (&= 1 (count xs)) (first xs)
                  quasiquote
                    &let nil ~@xs
                apply-args (inner-body args)
                  fn (body ys)
                    if (empty? ys)
                      quote-replace ~body
                      &let
                        a0 (last ys)
                        &let
                          code
                            [] (quote-replace defn) (turn-symbol (&str-concat |f_ (turn-string a0))) ([] a0) body
                          recur code (butlast ys)

        |has-index? $ quote
          defn has-index? (xs idx)
            assert "|expects a list" (list? xs)
            assert "|expects list key to be a number" (number? idx)
            assert "|expects list key to be an integer" (&= idx (floor idx))
            if
              &> idx 0
              &< idx (count xs)
              , false

        |update $ quote
          defn update (x k f)
            cond
              (list? x)
                if (has-index? x k)
                  assoc x k $ f (nth x k)
                  , x
              (map? x)
                if (contains? x k)
                  assoc x k $ f (&get x k)
                  , x
              (record? x)
                if (contains? x k)
                  assoc x k $ f (&get x k)
                  , x
              true
                raise $ &str-concat "|Cannot update key on item: " x

        |group-by $ quote
          defn group-by (xs0 f)
            apply-args
              ({}) xs0
              fn (acc xs)
                if (empty? xs) acc
                  let
                      x0 $ first xs
                      key $ f x0
                    recur
                      if (contains? acc key)
                        update acc key $ \ append % x0
                        assoc acc key $ [] x0
                      rest xs

        |keys $ quote
          defn keys (x)
            map (to-pairs x) first

        |keys-non-nil $ quote
          defn keys-non-nil (x)
            apply-args
              (#{}) (to-pairs x)
              fn (acc pairs)
                if (empty? pairs) acc
                  &let
                    pair $ first pairs
                    if (nil? (last pair))
                      recur acc (rest pairs)
                      recur (include acc (first pair))
                        rest pairs

        |vals $ quote
          defn vals (x)
            map (to-pairs x) last

        |frequencies $ quote
          defn frequencies (xs0)
            assert "|expects a list for frequencies" (list? xs0)
            apply-args
              ({}) xs0
              fn (acc xs)
                &let
                  x0 (first xs)
                  if (empty? xs) acc
                    recur
                      if (contains? acc (first xs))
                        update acc (first xs) (\ &+ % 1)
                        assoc acc (first xs) 1
                      rest xs

        |section-by $ quote
          defn section-by (xs0 n)
            apply-args
              ([]) xs0
              fn (acc xs)
                if (&<= (count xs) n)
                  append acc xs
                  recur
                    append acc (take xs n)
                    drop xs n

        |[][] $ quote
          defmacro [][] (& xs)
            &let
              items $ map xs
                fn (ys) $ quote-replace $ [] ~@ys
              quote-replace $ [] ~@items

        |{} $ quote
          defmacro {} (& xs)
            &let
              ys $ concat & xs
              quote-replace $ &{} ~@ys

        |js-object $ quote
          defmacro js-object (& xs)
            &let
              ys $ concat & xs
              quote-replace $ &js-object ~@ys

        |%{} $ quote
          defmacro %{} (R & xs)
            &let
              args $ concat & xs
              quote-replace $ &%{} ~R ~@args

        |fn $ quote
          defmacro fn (args & body)
            quote-replace $ defn f% ~args ~@body

        |assert= $ quote
          defmacro assert= (a b)
            &let
              va $ gensym |va
              &let
                vb $ gensym |vb
                quote-replace
                  &let
                    ~va ~a
                    &let
                      ~vb ~b
                      if (not= ~va ~vb)
                        &let nil
                          echo
                          echo "|Left: " ~va
                          echo "|      " $ format-to-lisp $ quote ~a
                          echo "|Right:" ~vb
                          echo "|      " $ format-to-lisp $ quote ~b
                          raise "|not equal in assertion!"

        |assert-detect $ quote
          defmacro assert-detect (f code)
            &let
              v $ gensym |v
              quote-replace
                &let
                  ~v ~code
                  if (~f ~v) nil
                    &let nil
                      echo
                      echo (format-to-lisp (quote ~code)) "|does not satisfy:" (format-to-lisp (quote ~f)) "| <--------"
                      echo "|  value is:" ~v
                      raise "|Not satisfied in assertion!"

        |swap! $ quote
          defmacro swap! (a f & args)
            quote-replace
              reset! ~a
                ~f (deref ~a) ~@args

        |assoc-in $ quote
          defn assoc-in (data path v)
            if (empty? path) v
              let
                  p0 $ first path
                  d $ either data $ &{}
                assoc d p0
                  assoc-in
                    if (contains? d p0) (get d p0) (&{})
                    rest path
                    , v

        |update-in $ quote
          defn update-in (data path f)
            if (empty? path)
              f data
              &let
                p0 $ first path
                assoc data p0
                  update-in (get data p0) (rest path) f

        |dissoc-in $ quote
          defn dissoc-in (data path)
            cond
              (empty? path) nil
              (&= 1 (count path))
                dissoc data (first path)
              true
                &let
                  p0 $ first path
                  assoc data p0
                    dissoc-in (get data p0) (rest path)

        |inc $ quote
          defn inc (x) $ &+ x 1

        |dec $ quote
          defn dec (x) $ &- x 1

        |starts-with? $ quote
          defn starts-with? (x y)
            &= 0 (str-find x y)

        |ends-with? $ quote
          defn ends-with? (x y)
            &=
              &- (count x) (count y)
              str-find x y

        |loop $ quote
          defmacro loop (pairs & body)
            assert "|expects pairs in loop" (list? pairs)
            assert "|expects pairs in pairs in loop"
              every? pairs
                defn detect-pairs? (x)
                  if (list? x)
                    &= 2 (count x)
                    , false
            let
                args $ map pairs first
                values $ map pairs last
              assert "|loop requires symbols in pairs" (every? args symbol?)
              quote-replace
                apply
                  defn generated-loop ~args ~@body
                  [] ~@values

        |let $ quote
          defmacro let (pairs & body)
            assert "|expects pairs in list for let" (list? pairs)
            if (&= 1 (count pairs))
              quote-replace
                &let
                  ~ $ nth pairs 0
                  ~@ body
              if (empty? pairs)
                quote-replace $ &let nil ~@body
                quote-replace
                  &let
                    ~ $ nth pairs 0
                    let
                      ~ $ rest pairs
                      ~@ body

        |let-sugar $ quote
          defmacro let-sugar (pairs & body)
            assert "|expects pairs in list for let" (list? pairs)
            if (empty? pairs)
              quote-replace $ &let nil ~@body
              &let
                pair $ first pairs
                assert "|expected pair length of 2" (&= 2 (count pair))
                if (&= 1 (count pairs))
                  quote-replace
                    let-destruct ~@pair
                      ~@ body
                  quote-replace
                    let-destruct ~@pair
                      let-sugar
                        ~ $ rest pairs
                        ~@ body

        |let-destruct $ quote
          defmacro let-destruct (pattern v & body)
            if (symbol? pattern)
              quote-replace
                &let (~pattern ~v) ~@body
              if (list? pattern)
                if (&= '[] (first pattern))
                  quote-replace
                    let[] (~ (rest pattern)) ~v ~@body
                  if (&= '{} (first pattern))
                    quote-replace
                      let{} (~ (rest pattern)) ~v ~@body
                    &let nil
                      echo pattern
                      raise "|Unknown pattern to destruct"
                raise "|Unknown structure to destruct"

        |[,] $ quote
          defmacro [,] (& body)
            &let
              xs $ filter body
                fn (x) (/= x ',)
              quote-replace $ [] ~@xs

        |assert $ quote
          defmacro assert (message xs)
            if
              if (string? xs) (not (string? message)) false
              quote-replace $ assert ~xs ~message
              quote-replace
                &let nil
                  if (not (string? ~message))
                    raise "|expects 1st argument to be string"
                  if ~xs nil
                    &let nil
                      echo "|Failed assertion:" (format-to-lisp (quote ~xs))
                      raise
                        ~ $ &str-concat (&str-concat message "| ") (format-to-lisp xs)

        |join-str $ quote
          defn join-str (xs0 sep)
            apply-args (| xs0 true)
              fn (acc xs beginning?)
                if (empty? xs) acc
                  recur
                    &str-concat
                      if beginning? acc $ &str-concat acc sep
                      first xs
                    rest xs
                    , false

        |join $ quote
          defn join (xs0 sep)
            apply-args
              ([]) xs0 true
              fn (acc xs beginning?)
                if (empty? xs) acc
                  recur
                    append
                      if beginning? acc (append acc sep)
                      first xs
                    rest xs
                    , false

        |repeat $ quote
          defn repeat (x n0)
            apply-args
              ([]) n0
              fn (acc n)
                if (&<= n 0) acc
                  recur (append acc x) (&- n 1)

        |interleave $ quote
          defn interleave (xs0 ys0)
            apply-args
              ([]) xs0 ys0
              fn (acc xs ys)
                if
                  if (empty? xs) true (empty? ys)
                  , acc
                  recur
                    -> acc (append (first xs)) (append (first ys))
                    rest xs
                    rest ys

        |map-kv $ quote
          defn map-kv (xs f)
            assert "|expects a map" (map? xs)
            foldl xs ({})
              fn (acc pair) $ let[] (k v) pair
                &let
                  result (f k v)
                  assert "|expected pair returned when mapping hashmap"
                    and (list? result) (&= 2 (count result))
                  let[] (k2 v2) result
                    assoc acc k2 v2

        |either $ quote
          defmacro either (x y)
            quote-replace $ if (nil? ~x) ~y ~x

        |def $ quote
          defmacro def (name x) x

        |and $ quote
          defmacro and (item & xs)
            if (empty? xs)
              quote-replace
                if ~item ~item false
              quote-replace
                if ~item
                  and
                    ~ $ first xs
                    ~@ $ rest xs
                  , false

        |or $ quote
          defmacro or (item & xs)
            if (empty? xs) item
              quote-replace
                if ~item ~item
                  or
                    ~ $ first xs
                    ~@ $ rest xs

        |with-log $ quote
          defmacro with-log (x)
            &let
              v $ gensym |v
              quote-replace
                &let
                  ~v ~x
                  echo (format-to-lisp (quote ~x)) |=> ~v
                  ~ v

        |with-js-log $ quote
          defmacro with-js-log (x)
            &let
              v $ gensym |v
              quote-replace
                &let
                  ~v ~x
                  js/console.log (format-to-lisp (quote ~x)) |=> ~v
                  ~ v

        |{,} $ quote
          defmacro {,} (& body)
            &let
              xs $ filter body
                fn (x) (not= x ',)
              quote-replace
                pairs-map $ section-by ([] ~@xs) 2

        |&doseq $ quote
          defmacro &doseq (pair & body)
            assert "|doseq expects a pair"
              if (list? pair) (&= 2 (count pair)) false
            let
                name $ first pair
                xs0 $ last pair
              quote-replace
                foldl ~xs0 nil $ defn doseq-fn% (_acc ~name) ~@body

        |with-cpu-time $ quote
          defmacro with-cpu-time (x)
            let
                started $ gensym |started
                v $ gensym |v
              quote-replace
                let
                    ~started (cpu-time)
                    ~v ~x
                  echo "|[cpu-time]" (format-to-lisp (quote ~x)) |=>
                    format-number
                      &- (cpu-time) ~started
                      , 3
                    , |ms
                  ~ v

        |call-with-log $ quote
          defmacro call-with-log (f & xs)
            let
                v $ gensym |v
                args-value $ gensym |args-value
              quote-replace
                let
                    ~args-value $ [] ~@xs
                    ~v $ ~f & ~args-value
                  echo "|call:"
                    format-to-lisp $ quote (call-with-log ~f ~@xs)
                    , |=> ~v
                  echo "|f:   " ~f
                  echo "|args:" ~args-value
                  ~ v

        |defn-with-log $ quote
          defmacro defn-with-log (f-name args & body)
            quote-replace
              defn ~f-name ~args
                &let
                  ~f-name $ defn ~f-name ~args ~@body
                  call-with-log ~f-name ~@args

        |do $ quote
          defmacro do (& body)
            ; echo "|body:" (format-to-lisp body)
            assert "|empty do is not okay" $ not $ empty? body
            quasiquote
              &let nil
                ~@ body

        |let{} $ quote
          defmacro let{} (items base & body)
            assert (str "|expects symbol names in binding names: " items)
              if (list? items) (every? items symbol?) false
            let
                var-result $ gensym |result
              quote-replace
                &let
                  ~var-result ~base
                  assert (str "|expected map for destructing: " ~var-result) (map? ~var-result)
                  let
                    ~ $ map items
                      defn gen-items% (x)
                        [] x ([] (turn-keyword x) var-result)
                    ~@ body

        |let[] $ quote
          defmacro let[] (vars data & body)
            assert "|expects a list of definitions"
              if (list? vars)
                every? vars symbol?
                , false
            let
                v $ gensym |v
                defs $ apply-args
                  [] ([]) vars 0
                  defn let[]% (acc xs idx)
                    if (empty? xs) acc
                      &let nil
                        when-not
                          symbol? (first xs)
                          raise $ &str-concat "|Expected symbol for vars: " (first xs)
                        if (&= (first xs) '&)
                          &let nil
                            assert "|expected list spreading" (&= 2 (count xs))
                            conj acc $ [] (get xs 1) (quote-replace (slice ~v ~idx))
                          recur
                            conj acc $ [] (first xs) (quote-replace (get ~v ~idx))
                            rest xs
                            inc idx
              quote-replace
                &let
                  ~v ~data
                  let
                    ~ defs
                    ~@ body

        |defrecord $ quote
          defmacro defrecord (name & xs)
            quote-replace
              new-record (quote ~name) ~@xs

        |defrecord! $ quote
          defmacro defrecord! (name & pairs)
            quasiquote
              %{} (new-record (quote ~name) (~@ (map pairs first))) ~@pairs

        |;nil $ quote
          defmacro ;nil (& _body) nil

        |strip-prefix $ quote
          defn strip-prefix (s piece)
            if (starts-with? s piece)
              substr s (count piece)
              , s

        |strip-suffix $ quote
          defn strip-suffix (s piece)
            if (ends-with? s piece)
              substr s 0 (&- (count s) (count piece))
              , s

        |select-keys $ quote
          defn select-keys (m xs)
            assert "|expectd map for selecting" $ map? m
            foldl xs (&{}) $ fn (acc k)
              assoc acc k (&get m k)

        |unselect-keys $ quote
          defn unselect-keys (m xs)
            assert "|expectd map for unselecting" $ map? m
            foldl xs m $ fn (acc k)
              dissoc acc k

        |conj $ quote
          defn conj (xs a)
            append xs a

        |turn-str $ quote
          defn turn-str (x) (turn-string x)

        |reduce $ quote
          defn reduce (xs x0 f)
            foldl xs x0 f

        |dbt $ quote
          def dbt dual-balanced-ternary

        |/= $ quote
          defn /= (a b) (not= a b)

        |invoke $ quote
          defn invoke (pair name & params)
            assert "|method! applies on a pair, leading a record"
              and (list? pair) (= 2 (count pair)) (record? (first pair))
            assert "|method by string or keyword"
              or (string? name) (keyword? name) (symbol? name)
            let
                proto $ nth pair 0
                f $ &get proto name
              assert "|expected function" (fn? f)
              f (nth pair 1) & params

        |&core-number-class $ quote
          defrecord! &core-number-class
            :ceil ceil
            :floor floor
            :format format-number
            :inc inc
            :pow pow
            :round round
            :sqrt sqrt

        |&core-string-class $ quote
          defrecord! &core-string-class
            :blank? blank?
            :count count
            :empty empty
            :ends-with? ends-with?
            :get nth
            :parse-float parse-float
            :parse-json parse-json
            :replace replace
            :split split
            :split-lines split-lines
            :starts-with? starts-with?
            :strip-prefix strip-prefix
            :strip-suffix strip-suffix
            :substr substr
            :trim trim

        |&core-set-class $ quote
          defrecord! &core-set-class
            :add coll-append
            :count count
            :difference difference
            :exclude exclude
            :empty empty
            :empty? empty?
            :include include
            :includes? includes?
            :intersection intersection
            :to-list set->list
            :union union

        |&core-map-class $ quote
          defrecord! &core-map-class
            :assoc assoc
            :contains? contains?
            :count count
            :dissoc dissoc
            :empty empty
            :empty? empty?
            :get &get
            :get-in get-in
            :includes? includes?
            :keys keys
            :keys-non-nil keys-non-nil
            :map-kv map-kv
            :merge merge
            :select-keys select-keys
            :to-pairs to-pairs
            :unselect-keys unselect-keys

        |&core-record-class $ quote
          defrecord! &core-record-class
            :get-name get-record-name
            :same-kind? relevant-record?
            :turn-map turn-map

        |&core-list-class $ quote
          defrecord! &core-list-class
            :any? any?
            :add coll-append
            :append append
            :assoc assoc
            :assoc-after assoc-after
            :assoc-before assoc-before
            :butlast butlast
            :concat concat
            :count count
            :drop drop
            :each each
            :empty empty
            :empty? empty?
            :filter filter
            :filter-not filter-not
            :find-index find-index
            :foldl foldl
            :frequencies frequencies
            :get nth
            :get-in get-in
            :group-by group-by
            :has-index? has-index?
            :index-of index-of
            :interleave interleave
            :join join
            :map map
            :map-indexed map-indexed
            :max max
            :min min
            :nth nth
            :pairs-map pairs-map
            :prepend prepend
            :reduce reduce
            :rest rest
            :reverse reverse
            :section-by section-by
            :slice slice
            :sort sort
            :take take
            :zipmap zipmap

        |&init-builtin-classes! $ quote
          defn &init-builtin-classes! ()
            ; "this function to make sure builtin classes are loaded"
            identity &core-number-class
            identity &core-string-class
            identity &core-set-class
            identity &core-list-class
            identity &core-map-class
            identity &core-record-class
