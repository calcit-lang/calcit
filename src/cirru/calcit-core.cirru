
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
              nth xs
                &- (count xs) 1

        |when $ quote
          defmacro when (condition & body)
            quasiquote $ if ~condition (&let nil ~@body)

        |when-not $ quote
          defmacro when-not (condition & body)
            quasiquote $ if (not ~condition) (&let nil ~@body)

        |+ $ quote
          defn + (x & ys) $ reduce ys x &+

        |- $ quote
          defn - (x & ys)
            if (&list:empty? ys)
              &- 0 x
              reduce ys x &-

        |* $ quote
          defn * (x & ys) $ reduce ys x &*

        |/ $ quote
          defn / (x & ys)
            if (&list:empty? ys)
              &/ 1 x
              reduce ys x &/

        |foldl-compare $ quote
          defn foldl-compare (xs acc f)
            if (&list:empty? xs) true
              if (f acc (&list:first xs))
                recur (&list:rest xs) (&list:first xs) f
                , false

        |foldl' $ quote
          defn foldl' (xs acc f)
            if (&list:empty? xs) acc
              recur (&list:rest xs) (f acc (&list:first xs)) f

        |< $ quote
          defn < (x & ys)
            if
              &= 1 (&list:count ys)
              &< x (&list:first ys)
              foldl-compare ys x &<

        |> $ quote
          defn > (x & ys)
            if
              &= 1 (&list:count ys)
              &> x (&list:first ys)
              foldl-compare ys x &>

        |= $ quote
          defn = (x & ys)
            if
              &= 1 (&list:count ys)
              &= x (&list:first ys)
              foldl-compare ys x &=

        |>= $ quote
          defn >= (x & ys)
            if
              &= 1 (&list:count ys)
              &>= x (&list:first ys)
              foldl-compare ys x &>=

        |<= $ quote
          defn <= (x & ys)
            if
              &= 1 (&list:count ys)
              &<= x (&list:first ys)
              foldl-compare ys x &<=

        |apply $ quote
          defn apply (f args) $ f & args

        |apply-args $ quote
          defmacro apply-args (args f)
            if (&= '[] (&list:first args))
              quasiquote
                ~f (~@ (&list:rest args))
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

        |buffer? $ quote
          defn buffer? (x) $ &= (type-of x) :buffer

        |fn? $ quote
          defn fn? (x)
            if
              &= (type-of x) :fn
              , true
              &= (type-of x) :proc

        |each $ quote
          defn each (xs f)
            foldl xs nil $ defn %each (_acc x)
              f x

        |&map:map $ quote
          defn &map:map (xs f)
            foldl xs ({})
              defn &map:map (acc pair)
                &let
                  result $ f pair
                  assert "|expected pair returned when mapping hashmap"
                    and (list? result) (&= 2 (&list:count result))
                  &map:assoc acc (nth result 0) (nth result 1)

        |&list:map $ quote
          defn &list:map (xs f)
            foldl xs ([])
              defn %&list:map (acc x) $ append acc (f x)

        |map $ quote
          defn map (xs f)
            if (list? xs) (&list:map xs f)
              if (set? xs)
                foldl xs (#{})
                  defn %map (acc x) $ include acc (f x)
                if (map? xs) (&map:map xs f)
                  &let nil
                    println "|value:" xs
                    raise "|expected list or set for map function"

        |&map:map-list $ quote
          defn &map:map-list (xs f)
            if (map? xs)
              foldl xs ([])
                defn %&map:map-list (acc pair)
                  append acc $ f pair
              raise "|&map:map-list expected a map"

        |take $ quote
          defn take (xs n)
            if (>= n (&list:count xs)) xs
              slice xs 0 n

        |take-last $ quote
          defn take-last (xs n)
            if (>= n (&list:count xs)) xs
              slice xs (- (&list:count xs) n) (&list:count xs)

        |drop $ quote
          defn drop (xs n)
            slice xs n (&list:count xs)

        |slice $ quote
          defn slice (xs n ? m)
            if (nil? xs) nil
              .slice xs n m

        |str $ quote
          defn str (x0 & xs)
            if (&list:empty? xs)
              &str x0
              &str:concat x0 $ str & xs

        |&str-spaced $ quote
          defn &str-spaced (head? x0 & xs)
            if (&list:empty? xs)
              if head? (&str x0)
                if (nil? x0) |
                  &str:concat "| " x0
              if (some? x0)
                &str:concat
                  if head? (&str x0) $ &str:concat "| " x0
                  &str-spaced false & xs
                &str-spaced head? & xs

        |str-spaced $ quote
          defn str-spaced (& xs)
            &str-spaced true & xs

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
              fn (acc item) $ &set:intersection acc item

        |index-of $ quote
          defn index-of (xs item)
            foldl-shortcut xs 0 nil $ defn %index-of (idx x)
              if (&= item x)
                :: true idx
                :: false (&+ 1 idx)

        |find-index $ quote
          defn find-index (xs f)
            foldl-shortcut xs 0 nil $ defn %find-index (idx x)
              if (f x)
                :: true idx
                :: false (&+ 1 idx)

        |find $ quote
          defn find (xs f)
            foldl-shortcut xs 0 nil $ defn %find (_acc x)
              if (f x)
                :: true x
                :: false nil

        |&list:last-index-of $ quote
          defn &list:last-index-of (xs item)
            foldr-shortcut xs (dec $ count xs) nil $ fn (idx x)
              if (&= item x)
                :: true idx
                :: false (&- 1 idx)

        |&list:find-last-index $ quote
          defn &list:find-last-index (xs f)
            foldr-shortcut xs (dec $ count xs) nil $ fn (idx x)
              if (f x)
                :: true idx
                :: false (&- 1 idx)

        |&list:find-last $ quote
          defn &list:find-last (xs f)
            foldr-shortcut xs nil nil $ fn (_acc x)
              if (f x)
                :: true x
                :: false nil

        |-> $ quote
          defmacro -> (base & xs)
            if (&list:empty? xs)
              quasiquote ~base
              &let
                x0 (&list:first xs)
                if (list? x0)
                  recur
                    &list:concat ([] (&list:first x0) base) (&list:rest x0)
                    , & (&list:rest xs)
                  recur ([] x0 base) & (&list:rest xs)

        |->> $ quote
          defmacro ->> (base & xs)
            if (&list:empty? xs)
              quasiquote ~base
              &let
                x0 (&list:first xs)
                if (list? x0)
                  recur (append x0 base) & (&list:rest xs)
                  recur ([] x0 base) & (&list:rest xs)

        |->% $ quote
          defmacro ->% (base & xs)
            if (&list:empty? xs) base
              let
                  tail $ last xs
                  pairs $ &list:concat
                    [] $ [] '% base
                    map
                      butlast xs
                      defn %->% (x) ([] '% x)
                quasiquote
                  let ~pairs ~tail

        |%<- $ quote
          defmacro %<- (& xs)
            quasiquote $ ->% $ ~@ $ reverse xs

        |<- $ quote
          defmacro <- (& xs)
            quasiquote $ -> $ ~@ $ reverse xs

        |cond $ quote
          defmacro cond (pair & else)
            assert "|expects a pair"
              if (list? pair) (&= 2 (&list:count pair)) false
            &let
              expr $ &list:nth pair 0
              &let
                branch $ &list:nth pair 1
                if
                  if (empty? else) (= true expr) false
                  , branch
                  quasiquote
                    if ~expr ~branch
                      ~ $ if (&list:empty? else) nil
                        quasiquote
                          cond
                            ~ $ &list:nth else 0
                            ~@ $ &list:rest else

        |&key-match-internal $ quote
          defmacro key-match (value & body)
            if (&list:empty? body)
              quasiquote
                eprintln "|[Warn] key-match found no matched case, missing `_` case?" ~value
              &let
                pair (&list:first body)
                assert "|key-match expected pairs"
                  and (list? pair) (&= 2 (&list:count pair))
                let
                    pattern $ &list:nth pair 0
                    branch $ &list:nth pair 1
                  if (list? pattern)
                    &let
                      k (&list:first pattern)
                      quasiquote
                        if (&= (&list:first ~value) ~k)
                          let
                            ~ $ map-indexed (&list:rest pattern) $ defn %key-match (idx x)
                              [] x $ quasiquote
                                &list:nth ~value (~ (inc idx))
                            , ~branch
                          &key-match-internal ~value $ ~@ (&list:rest body)
                    if (&= pattern '_) branch
                      raise $ str "|unknown supported pattern: " pair

        |key-match $ quote
          defmacro key-match (value & body)
            if (&list:empty? body)
              quasiquote
                eprintln "|[Error] key-match expected some patterns and matches" ~value
              if (list? value)
                &let (v# (gensym |v))
                  quasiquote
                    &let (~v# ~value)
                      &key-match-internal ~v# $ ~@ body
                quasiquote
                  &key-match-internal ~value $ ~@ body

        |&case $ quote
          defmacro &case (item default pattern & others)
            assert "|`case` expects pattern in a pair"
              if (list? pattern) (&= 2 (&list:count pattern)) false
            let
                x $ &list:first pattern
                branch $ last pattern
              quasiquote
                if (&= ~item ~x) ~branch
                  ~ $ if (&list:empty? others) default
                    quasiquote
                      &case ~item ~default ~@others

        |case $ quote
          defmacro case (item & patterns)
            &let
              v (gensym |v)
              quasiquote
                &let
                  ~v ~item
                  &case ~v nil ~@patterns

        |case-default $ quote
          defmacro case (item default & patterns)
            if (&list:empty? patterns)
              raise "|Expected patterns for case-default, got empty"
            &let
              v (gensym |v)
              quasiquote
                &let (~v ~item)
                  &case ~v ~default ~@patterns

        |get $ quote
          defn get (base k)
            if (nil? base) nil
              if (string? base) (&str:nth base k)
                if (map? base) (&map:get base k)
                  if (list? base) (&list:nth base k)
                    if (tuple? base) (&tuple:nth base k)
                      if (record? base) (&record:get base k)
                        &let nil
                          eprintln "|Value:" base k
                          raise "|Expected map or list for get"

        |get-in $ quote
          defn get-in (base path)
            assert "|expects path in a list" (list? path)
            if (nil? base) nil
              if (&list:empty? path) base
                recur
                  get base (&list:first path)
                  rest path

        |&max $ quote
          defn &max (a b)
            assert "|expects numbers for &max"
              if (number? a) (number? b)
                if (string? a) (string? b) false
            if (&> a b) a b

        |&min $ quote
          defn &min (a b)
            assert "|expects numbers for &min"
              if (number? a) (number? b)
                if (string? a) (string? b) false
            if (&< a b) a b

        |&list:max $ quote
          defn &list:max (xs)
            if (&list:empty? xs) nil
              reduce (&list:rest xs) (&list:first xs)
                defn %max (acc x) (&max acc x)

        |&list:min $ quote
          defn &list:min (xs)
            if (&list:empty? xs) nil
              reduce (&list:rest xs) (&list:first xs)
                defn %min (acc x) (&min acc x)

        |&set:max $ quote
          defn &set:max (xs)
            if (&set:empty? xs) nil
              reduce (&set:rest xs) (&set:first xs)
                defn %max (acc x) (&max acc x)

        |&set:min $ quote
          defn &set:min (xs)
            if (&set:empty? xs) nil
              reduce (&set:rest xs) (&set:first xs)
                defn %min (acc x) (&min acc x)

        |max $ quote
          defn max (xs)
            .max xs

        |min $ quote
          defn min (xs)
            .min xs

        |every? $ quote
          defn every? (xs f)
            foldl-shortcut xs nil true $ defn %every? (_acc x)
              if (f x)
                :: false nil
                :: true false

        |any? $ quote
          defn any? (xs f)
            foldl-shortcut xs nil false $ defn %any? (_acc x)
              if (f x)
                :: true true
                :: false nil

        |mapcat $ quote
          defn mapcat (xs f)
            &list:concat & $ map xs f

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
            foldl xs ([]) $ defn %map-indexed (acc x)
              append acc $ f (count acc) x

        |filter $ quote
          defn filter (xs f)
            .filter xs f

        |filter-not $ quote
          defn filter-not (xs f)
            .filter xs $ defn %filter-not (x) $ not $ f x

        |&map:filter $ quote
          defn &map:filter (xs f)
            reduce xs (&{})
              defn %&map:filter (acc x)
                if (f x) (&map:assoc acc (nth x 0) (nth x 1)) acc

        |&map:filter-kv $ quote
          defn &map:filter-kv (xs f)
            reduce xs (&{})
              defn %map:filter-kv (acc x)
                if
                  f (nth x 0) (nth x 1)
                  &map:assoc acc (nth x 0) (nth x 1)
                  , acc

        |&map:add-entry $ quote
          defn &map:add-entry (xs pair)
            assert "|&map:add-entry expected value in a pair" $ and (list? pair)
              &= 2 (count pair)
            &map:assoc xs (nth pair 0) (nth pair 1)

        |&list:filter $ quote
          defn &list:filter (xs f)
            reduce xs ([])
              defn %&list:filter (acc x)
                if (f x) (append acc x) acc

        |&set:filter $ quote
          defn &set:filter (xs f)
            reduce xs (#{})
              defn %&set:filter (acc x)
                if (f x) (&include acc x) acc

        |empty $ quote
          defn empty (x)
            if (nil? x) nil
              if (list? x) ([])
                .empty x

        |pairs-map $ quote
          defn pairs-map (xs)
            reduce xs ({})
              defn %pairs-map (acc pair)
                assert "|expects pair for pairs-map"
                  if (list? pair)
                    &= 2 (&list:count pair)
                    , false
                &map:assoc acc (&list:first pair) (last pair)

        |some? $ quote
          defn some? (x) $ not $ nil? x

        |some-in? $ quote
          defn some-in? (x path)
            if (nil? x) false
              if (&list:empty? path) true
                &let (k $ &list:first path)
                  if (map? x)
                    if (contains? x k)
                      recur (get x k) (&list:rest path)
                      , false
                    if (list? x)
                      if (number? k)
                        recur (get x k) (&list:rest path)
                        , false
                      raise $ &str:concat "|Unknown structure for some-in? detection: " x


        |zipmap $ quote
          defn zipmap (xs0 ys0)
            apply-args
              ({}) xs0 ys0
              fn (acc xs ys)
                if
                  if (&list:empty? xs) true (&list:empty? ys)
                  , acc
                  recur
                    &map:assoc acc (&list:first xs) (&list:first ys)
                    rest xs
                    rest ys

        |contains-symbol? $ quote
          defn contains-symbol? (xs y)
            if (list? xs)
              apply-args (xs)
                defn %contains-symbol? (body)
                  if (&list:empty? body) false
                    if
                      contains-symbol? (&list:first body) y
                      , true
                      recur (&list:rest body)
              &= xs y

        |\ $ quote
          defmacro \ (& xs)
            quasiquote $ defn %\ (? % %2) ~xs

        |\. $ quote
          defmacro \. (args-alias & xs)
            &let
              args $ ->% (turn-string args-alias) (split % |.) (map % turn-symbol)
              &let
                inner-body $ if (&= 1 (&list:count xs)) (&list:first xs)
                  quasiquote
                    &let nil ~@xs
                apply-args (inner-body args)
                  fn (body ys)
                    if (&list:empty? ys)
                      quasiquote ~body
                      &let
                        a0 (last ys)
                        &let
                          code
                            [] (quasiquote defn) (turn-symbol (&str:concat |f_ (turn-string a0))) ([] a0) body
                          recur code (butlast ys)

        |update $ quote
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
                  if (or (&= k 0) (&= k 1))
                    assoc x k $ f (&tuple:nth x k)
                    raise $ &str:concat "|tuple only has 0,1 fields, unknown field: " k
                  if (record? x)
                    if (contains? x k)
                      assoc x k $ f (&record:get x k)
                      , x
                    raise $ &str:concat "|Cannot update key on item: " (pr-str x)

        |group-by $ quote
          defn group-by (xs0 f)
            apply-args
              ({}) xs0
              defn %group-by (acc xs)
                if (&list:empty? xs) acc
                  let
                      x0 $ &list:first xs
                      key $ f x0
                    recur
                      if (contains? acc key)
                        update acc key $ \ append % x0
                        &map:assoc acc key $ [] x0
                      rest xs

        |keys $ quote
          defn keys (x)
            map (to-pairs x) &list:first

        |keys-non-nil $ quote
          defn keys-non-nil (x)
            apply-args
              (#{}) (to-pairs x)
              fn (acc pairs)
                if (&set:empty? pairs) acc
                  &let
                    pair $ &set:first pairs
                    if (nil? (last pair))
                      recur acc (&set:rest pairs)
                      recur (include acc (&list:first pair))
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
                  x0 (&list:first xs)
                  if (&list:empty? xs) acc
                    recur
                      if (contains? acc (&list:first xs))
                        update acc (&list:first xs) (\ &+ % 1)
                        &map:assoc acc (&list:first xs) 1
                      rest xs

        |section-by $ quote
          defn section-by (xs0 n)
            if (>= n 1)
              apply-args
                ([]) xs0
                fn (acc xs)
                  if (&<= (&list:count xs) n)
                    if (&list:empty? xs) acc
                      append acc xs
                    recur
                      append acc (take xs n)
                      drop xs n
              raise "|expected positive number"

        |[][] $ quote
          defmacro [][] (& xs)
            &let
              items $ map xs
                fn (ys) $ quasiquote $ [] ~@ys
              quasiquote $ [] ~@items

        |{} $ quote
          defmacro {} (& xs)
            &let
              ys $ &list:concat & xs
              quasiquote $ &{} ~@ys

        |js-object $ quote
          defmacro js-object (& xs)
            &let
              ys $ &list:concat & xs
              quasiquote $ &js-object ~@ys

        |%{} $ quote
          defmacro %{} (R & xs)
            &let
              args $ &list:concat & xs
              quasiquote $ &%{} ~R ~@args

        |fn $ quote
          defmacro fn (args & body)
            quasiquote $ defn f% ~args ~@body

        |assert= $ quote
          defmacro assert= (a b)
            &let
              va $ gensym |va
              &let
                vb $ gensym |vb
                quasiquote
                  &let
                    ~va ~a
                    &let
                      ~vb ~b
                      if (not= ~va ~vb)
                        &let nil
                          eprintln
                          eprintln "|Left: " ~va
                          eprintln "|      " $ format-to-lisp $ quote ~a
                          eprintln "|Right:" ~vb
                          eprintln "|      " $ format-to-lisp $ quote ~b
                          raise "|not equal in assertion!"

        |assert-detect $ quote
          defmacro assert-detect (f code)
            &let
              v $ gensym |v
              quasiquote
                &let
                  ~v ~code
                  if (~f ~v) nil
                    &let nil
                      eprintln
                      eprintln (format-to-lisp (quote ~code)) "|does not satisfy:" (format-to-lisp (quote ~f)) "| <--------"
                      eprintln "|  value is:" ~v
                      raise "|Not satisfied in assertion!"

        |swap! $ quote
          defmacro swap! (a f & args)
            quasiquote
              reset! ~a
                ~f (deref ~a) ~@args

        |assoc-in $ quote
          defn assoc-in (data path v)
            if (&list:empty? path) v
              &let
                p0 $ &list:first path
                &let
                  d $ either data $ &{}
                  assoc d p0
                    assoc-in
                      if (contains? d p0) (get d p0) (&{})
                      rest path
                      , v

        |update-in $ quote
          defn update-in (data path f)
            if (&list:empty? path)
              f data
              &let
                p0 $ &list:first path
                assoc data p0
                  update-in (get data p0) (&list:rest path) f

        |dissoc-in $ quote
          defn dissoc-in (data path)
            if (&list:empty? path) nil
              if (&= 1 (&list:count path))
                dissoc data (&list:first path)
                &let
                  p0 $ &list:first path
                  assoc data p0
                    dissoc-in (get data p0) (&list:rest path)

        |inc $ quote
          defn inc (x) $ &+ x 1

        |dec $ quote
          defn dec (x) $ &- x 1

        |starts-with? $ quote
          defn starts-with? (x y)
            &= 0 (&str:find-index x y)

        |ends-with? $ quote
          defn ends-with? (x y)
            &=
              &- (&str:count x) (&str:count y)
              &str:find-index x y

        |loop $ quote
          defmacro loop (pairs & body)
            assert "|expects pairs in loop" (list? pairs)
            assert "|expects pairs in pairs in loop"
              every? pairs
                defn detect-pairs? (x)
                  if (list? x)
                    &= 2 (&list:count x)
                    , false
            let
                args $ map pairs &list:first
                values $ map pairs last
              assert "|loop requires symbols in pairs" (every? args symbol?)
              quasiquote
                apply
                  defn generated-loop ~args ~@body
                  [] ~@values

        |let $ quote
          defmacro let (pairs & body)
            assert "|expects pairs in list for let" (list? pairs)
            if (&= 1 (&list:count pairs))
              quasiquote
                &let
                  ~ $ &list:nth pairs 0
                  ~@ body
              if (&list:empty? pairs)
                quasiquote $ &let nil ~@body
                quasiquote
                  &let
                    ~ $ &list:nth pairs 0
                    let
                      ~ $ &list:rest pairs
                      ~@ body

        |let-sugar $ quote
          defmacro let-sugar (pairs & body)
            assert "|expects pairs in list for let" (list? pairs)
            if (&list:empty? pairs)
              quasiquote $ &let nil ~@body
              &let
                pair $ &list:first pairs
                assert "|expected pair length of 2" (&= 2 (&list:count pair))
                if (&= 1 (&list:count pairs))
                  quasiquote
                    let-destruct ~@pair
                      ~@ body
                  quasiquote
                    let-destruct ~@pair
                      let-sugar
                        ~ $ &list:rest pairs
                        ~@ body

        |let-destruct $ quote
          defmacro let-destruct (pattern v & body)
            if (symbol? pattern)
              quasiquote
                &let (~pattern ~v) ~@body
              if (list? pattern)
                if (&= '[] (&list:first pattern))
                  quasiquote
                    let[] (~ (&list:rest pattern)) ~v ~@body
                  if (&= '{} (&list:first pattern))
                    quasiquote
                      let{} (~ (&list:rest pattern)) ~v ~@body
                    &let nil
                      eprintln pattern
                      raise "|Unknown pattern to destruct"
                raise "|Unknown structure to destruct"

        |when-let $ quote
          defmacro when-let (pair & body)
            assert "|expected a pair"
              and
                list? pair
                &= 2 $ count pair
            &let
              x $ nth pair 0
              assert "|expected a symbol for var name" (symbol? x)
              quasiquote $ &let
                ~x $ ~ $ nth pair 1
                if (some? ~x)
                  do $ ~@ body

        |if-let $ quote
          defmacro if-let (pair then ? else)
            assert "|expected a pair"
              and
                list? pair
                &= 2 $ count pair
            &let
              x $ nth pair 0
              assert "|expected a symbol for var name" (symbol? x)
              quasiquote $ &let
                ~x $ ~ $ nth pair 1
                if (some? ~x) ~then ~else

        |[,] $ quote
          defmacro [,] (& body)
            &let
              xs $ &list:filter body
                fn (x) (/= x ',)
              quasiquote $ [] ~@xs

        |assert $ quote
          defmacro assert (message xs)
            if
              if (string? xs) (not (string? message)) false
              quasiquote $ assert ~xs ~message
              quasiquote
                &let nil
                  if (not (string? ~message))
                    raise "|expects 1st argument to be string"
                  if ~xs nil
                    &let nil
                      eprintln "|Failed assertion:" (format-to-lisp (quote ~xs))
                      raise
                        ~ $ &str:concat (&str:concat message "| ") (format-to-lisp xs)

        |join-str $ quote
          defn join-str (xs0 sep)
            apply-args (| xs0 true)
              defn %join-str (acc xs beginning?)
                if (&list:empty? xs) acc
                  recur
                    &str:concat
                      if beginning? acc $ &str:concat acc sep
                      &list:first xs
                    &list:rest xs
                    , false

        |join $ quote
          defn join (xs0 sep)
            apply-args
              ([]) xs0 true
              defn %join (acc xs beginning?)
                if (&list:empty? xs) acc
                  recur
                    append
                      if beginning? acc (append acc sep)
                      &list:first xs
                    &list:rest xs
                    , false

        |repeat $ quote
          defn repeat (x n0)
            apply-args
              ([]) n0
              defn %repeat (acc n)
                if (&<= n 0) acc
                  recur (append acc x) (&- n 1)

        |interleave $ quote
          defn interleave (xs0 ys0)
            apply-args
              ([]) xs0 ys0
              defn %interleave (acc xs ys)
                if
                  if (&list:empty? xs) true (&list:empty? ys)
                  , acc
                  recur
                    -> acc (append (&list:first xs)) (append (&list:first ys))
                    rest xs
                    rest ys

        |map-kv $ quote
          defn map-kv (xs f)
            assert "|expects a map" (map? xs)
            foldl xs ({})
              defn %map-kv (acc pair)
                &let
                  result (f (nth pair 0) (nth pair 1))
                  if (list? result)
                    do
                      assert "|expected pair returned when mapping hashmap"
                        &= 2 (&list:count result)
                      &map:assoc acc (nth result 0) (nth result 1)
                    if (nil? result) acc
                      raise $ str "|map-kv expected list or nil, got: " result

        |either $ quote
          defmacro either (item & xs)
            if (&list:empty? xs) item
              if (list? item)
                &let (v1# (gensym |v1))
                  quasiquote
                    &let (~v1# ~item)
                      if (nil? ~v1#)
                        either
                          ~ $ &list:first xs
                          ~@ $ &list:rest xs
                        ~ v1#
                quasiquote
                  if (nil? ~item)
                    either
                      ~ $ &list:first xs
                      ~@ $ &list:rest xs
                    ~ item

        |def $ quote
          defmacro def (name x) x

        |and $ quote
          defmacro and (item & xs)
            if (&list:empty? xs)
              if (list? item)
                &let (v1# $ gensym |v1)
                  quasiquote
                    &let (~v1# ~item)
                      if ~v1# ~v1# false
                quasiquote
                  if ~item ~item false
              quasiquote
                if ~item
                  and
                    ~ $ &list:first xs
                    ~@ $ &list:rest xs
                  , false

        |or $ quote
          defmacro or (item & xs)
            if (&list:empty? xs) item
              if (list? item)
                &let (v1# (gensym |v1))
                  quasiquote
                    &let (~v1# ~item)
                      if (nil? ~v1#)
                        or
                          ~ $ &list:first xs
                          ~@ $ &list:rest xs
                        if (= false ~v1#)
                          or
                            ~ $ &list:first xs
                            ~@ $ &list:rest xs
                          ~ v1#
                quasiquote
                  if (nil? ~item)
                    or
                      ~ $ &list:first xs
                      ~@ $ &list:rest xs
                    if (= false ~item)
                      or
                        ~ $ &list:first xs
                        ~@ $ &list:rest xs
                      ~ item

        |w-log $ quote
          defmacro w-log (x)
            &let
              v $ if (= :eval $ &get-calcit-running-mode) (gensym |v) '_log_tmp
              if (list? x)
                quasiquote
                  &let
                    ~v ~x
                    println (format-to-lisp (quote ~x)) |=> ~v
                    ~ v
                quasiquote
                  &let nil
                    println (format-to-lisp (quote ~x)) |=> ~x
                    ~ x

        |wo-log $ quote
          defmacro wo-log (x) x

        |w-js-log $ quote
          defmacro w-js-log (x)
            if (list? x)
              &let
                v $ if (= :eval $ &get-calcit-running-mode) (gensym |v) '_log_tmp
                quasiquote
                  &let
                    ~v ~x
                    js/console.log (format-to-lisp (quote ~x)) |=> ~v
                    ~ v
              quasiquote
                &let nil
                  js/console.log (format-to-lisp (quote ~x)) |=> ~x
                  ~ x

        |wo-js-log $ quote
          defmacro w-js-log (x) x

        |{,} $ quote
          defmacro {,} (& body)
            &let
              xs $ &list:filter body
                defn %{,} (x) (not= x ',)
              quasiquote
                pairs-map $ section-by ([] ~@xs) 2

        |&doseq $ quote
          defmacro &doseq (pair & body)
            assert "|doseq expects a pair"
              if (list? pair) (&= 2 (&list:count pair)) false
            let
                name $ &list:first pair
                xs0 $ last pair
              quasiquote
                foldl ~xs0 nil $ defn doseq-fn% (_acc ~name) ~@body

        |with-cpu-time $ quote
          defmacro with-cpu-time (x)
            let
                started $ gensym |started
                v $ gensym |v
              quasiquote
                let
                    ~started (cpu-time)
                    ~v ~x
                  println "|[cpu-time]" (format-to-lisp (quote ~x)) |=>
                    .format
                      &- (cpu-time) ~started
                      , 3
                    , |ms
                  ~ v

        |call-w-log $ quote
          defmacro call-w-log (f & xs)
            let
                v $ if (= :eval $ &get-calcit-running-mode) (gensym |v) '_log_tmp
                args-value $ gensym |args-value
              quasiquote
                let
                    ~args-value $ [] ~@xs
                    ~v $ ~f & ~args-value
                  println "|call:"
                    format-to-lisp $ quote (call-w-log ~f ~@xs)
                    , |=> ~v
                  println "|f:   " ~f
                  println "|args:" ~args-value
                  ~ v

        |call-wo-log $ quote
          defmacro call-wo-log (f & xs)
            quasiquote $ ~f ~@xs

        |defn-w-log $ quote
          defmacro defn-w-log (f-name args & body)
            quasiquote
              defn ~f-name ~args
                &let
                  ~f-name $ defn ~f-name ~args ~@body
                  call-w-log ~f-name ~@args

        |defn-wo-log $ quote
          defmacro defn-wo-log (f-name args & body)
            quasiquote
              defn ~f-name ~args ~@body

        |do $ quote
          defmacro do (& body)
            ; println "|body:" (format-to-lisp body)
            assert "|empty do is not okay" $ not $ empty? body
            quasiquote
              &let nil
                ~@ body

        |let{} $ quote
          defmacro let{} (items base & body)
            assert (str "|expects symbol names in binding names: " items)
              if (list? items) (every? items symbol?) false
            &let
              var-result $ gensym |result
              quasiquote
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
                variable? $ symbol? data
                v $ if variable? data $ gensym |v
                defs $ apply-args
                  [] ([]) vars 0
                  defn let[]% (acc xs idx)
                    if (&list:empty? xs) acc
                      &let nil
                        when-not
                          symbol? (&list:first xs)
                          raise $ &str:concat "|Expected symbol for vars: " (&list:first xs)
                        if (&= (&list:first xs) '&)
                          &let nil
                            assert "|expected list spreading" (&= 2 (&list:count xs))
                            append acc $ [] (&list:nth xs 1) (quasiquote (&list:slice ~v ~idx))
                          recur
                            append acc $ [] (&list:first xs) (quasiquote (&list:nth ~v ~idx))
                            rest xs
                            inc idx

              if variable?
                quasiquote
                  let
                    ~ defs
                    ~@ body
                quasiquote
                  &let
                    ~v ~data
                    let
                      ~ defs
                      ~@ body

        |defrecord $ quote
          defmacro defrecord (name & xs)
            quasiquote
              new-record (~ (turn-keyword name)) ~@xs

        |defrecord! $ quote
          defmacro defrecord! (name & pairs)
            quasiquote
              %{} (new-record (~ (turn-keyword name)) (~@ (map pairs &list:first))) ~@pairs

        |;nil $ quote
          defmacro ;nil (& _body) nil

        |flipped $ quote
          defmacro flipped (f & args)
            quasiquote
              ~f $ ~@ $ reverse args

        |strip-prefix $ quote
          defn strip-prefix (s piece)
            if (starts-with? s piece)
              &str:slice s (&str:count piece)
              , s

        |strip-suffix $ quote
          defn strip-suffix (s piece)
            if (ends-with? s piece)
              &str:slice s 0 (&- (&str:count s) (&str:count piece))
              , s

        |select-keys $ quote
          defn select-keys (m xs)
            assert "|expectd map for selecting" $ map? m
            foldl xs (&{}) $ defn %select-keys (acc k)
              &map:assoc acc k (&map:get m k)

        |unselect-keys $ quote
          defn unselect-keys (m xs)
            assert "|expectd map for unselecting" $ map? m
            foldl xs m $ defn %unselect-keys (acc k)
              &map:dissoc acc k

        |conj $ quote
          defn conj (xs y0 & ys)
            if (empty? ys) (append xs y0)
              recur
                append xs y0
                , & ys

        |turn-str $ quote
          defn turn-str (x) (turn-string x)

        |reduce $ quote
          defn reduce (xs x0 f)
            foldl xs x0 f


        |/= $ quote
          defn /= (a b) (not= a b)

        |invoke $ quote
          defn invoke (pair name & params)
            assert "|method! applies on a pair, leading a record"
              and (list? pair) (= 2 (&list:count pair)) (record? (&list:first pair))
            assert "|method by string or keyword"
              or (string? name) (keyword? name) (symbol? name)
            let
                proto $ &tuple:nth pair 0
                f $ &record:get proto name
              assert "|expected function" (fn? f)
              f pair & params

        |&list:sort-by $ quote
          defn &list:sort-by (xs f)
            if (keyword? f)
              sort xs $ defn %&list:sort-by (a b)
                &compare (get a f) (get b f)

              sort xs $ defn %&list:sort-by (a b)
                &compare (f a) (f b)

        |negate $ quote
          defn negate (x)
            &- 0 x

        |reverse $ quote
          defn reverse (x)
            &list:reverse x

        |distinct $ quote
          defn distinct (x)
            &list:distinct x

        |&core-number-class $ quote
          defrecord! &core-number-class
            :ceil ceil
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

        |&core-string-class $ quote
          defrecord! &core-string-class
            :blank? blank?
            :count &str:count
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

        |&core-set-class $ quote
          defrecord! &core-set-class
            :add include
            :count &set:count
            :difference difference
            :exclude exclude
            :empty $ defn &set:empty (x) (#{})
            :empty? &set:empty?
            :filter &set:filter
            :include include
            :includes? &set:includes?
            :contains? &set:includes?
            :intersection intersection
            :max &set:max
            :min &set:min
            :to-list &set:to-list
            :union union
            :first &set:first
            :rest &set:rest
            :to-set identity
            :mappend union

        |&core-map-class $ quote
          defrecord! &core-map-class
            :add &map:add-entry
            :assoc &map:assoc
            :contains? &map:contains?
            :count &map:count
            :dissoc &map:dissoc
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
            :to-pairs to-pairs
            :values vals
            :first &map:first
            :rest &map:rest
            :diff-new &map:diff-new
            :diff-keys &map:diff-keys
            :common-keys &map:common-keys
            :to-map identity

        |&core-record-class $ quote
          defrecord! &core-record-class
            :get &record:get
            :get-name &record:get-name
            :matches? &record:matches?
            :to-map &record:to-map
            :count &record:count
            :contains? &record:contains?
            :assoc &record:assoc
            :from-map &record:from-map
            :extend-as &record:extend-as

        |&core-list-class $ quote
          defrecord! &core-list-class
            :any? any?
            :add append
            :append append
            :assoc &list:assoc
            :assoc-after &list:assoc-after
            :assoc-before &list:assoc-before
            :bind mapcat
            :butlast butlast
            :concat &list:concat
            :contains? &list:contains?
            :includes? &list:includes?
            :count &list:count
            :drop drop
            :each each
            :empty $ defn &list:empty (x) ([])
            :empty? &list:empty?
            :filter &list:filter
            :filter-not filter-not
            :find find
            :find-index find-index
            :find-last &list:find-last
            :find-last-index &list:find-last-index
            :foldl $ defn foldl (xs v0 f) (foldl xs v0 f)
            :get &list:nth
            :get-in get-in
            :group-by group-by
            :index-of index-of
            :join join
            :join-str join-str
            :last-index-of &list:last-index-of
            :map &list:map
            :map-indexed map-indexed
            :mappend $ defn &list:mappend (x y) $ &list:concat x y
            :max &list:max
            :min &list:min
            :nth &list:nth
            :pairs-map pairs-map
            :prepend prepend
            :reduce reduce
            :reverse &list:reverse
            :slice &list:slice
            :sort $ defn sort (x y) (sort x y)
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
              &list:concat &
                map fs $ defn &fn:ap-gen (f)
                  map xs $ defn &fn:ap-gen (x)
                    f x
            :flatten &list:flatten

        |&core-nil-class $ quote
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

        |&core-fn-class $ quote
          defrecord! &core-fn-class
            :call $ defn &fn:call (f & args) (f & args)
            :call-args $ defn &fn:call-args (f args) (f & args)
            :map $ defn &fn:map (f g) $ defn &fn:map (x) $ f (g x)
            :bind $ defn &fn:bind (m f) $ defn %&fn:bind (x) $ f (m x) x
            :mappend $ defn &fn:mappend (f g)
              defn %&fn:mappend (x) $ .mappend (f x) (g x)
            :apply $ defn &fn:apply (f g)
              defn %*fn:apply (x)
                g x (f x)

        |&init-builtin-classes! $ quote
          defn &init-builtin-classes! ()
            ; "this function to make sure builtin classes are loaded"
            identity &core-number-class
            identity &core-string-class
            identity &core-set-class
            identity &core-list-class
            identity &core-map-class
            identity &core-record-class
            identity &core-nil-class
            identity &core-fn-class

        |count $ quote
          defn count (x)
            if (nil? x) 0
              if (tuple? x) 2
                if (list? x)
                  &list:count x
                  .count x

        |empty? $ quote
          defn empty? (x)
            if (nil? x) true
              if (list? x)
                &list:empty? x
                .empty? x

        |contains? $ quote
          defn contains? (x k)
            if (nil? x) false
              if (list? x) (&list:contains? x k)
                if (tuple? x)
                  or (&= k 0) (&= k 1)
                  .contains? x k

        |contains-in? $ quote
          defn contains-in? (xs path)
            if (empty? path) true
              &let
                p0 $ first path
                cond
                  (list? xs)
                    if
                      and (number? p0) (&list:contains? xs p0)
                      recur (nth xs p0) (rest path)
                      , false
                  (map? xs)
                    if (&map:contains? xs p0)
                      recur (&map:get xs p0) (rest path)
                      , false
                  (record? xs)
                    if (&record:contains? xs p0)
                      recur (&record:get xs p0) (rest path)
                      , false
                  (tuple? xs)
                    or (&= p0 0) (&= p0 1)
                  true false

        |includes? $ quote
          defn includes? (x k)
            if (nil? x) false
              if (list? x) (&list:includes? x k)
                .includes? x k

        |nth $ quote
          defn nth (x i)
            if (tuple? x) (&tuple:nth x i)
              if (list? x) (&list:nth x i)
                .nth x i

        |first $ quote
          defn first (x)
            if (nil? x) nil
              if (tuple? x) (&tuple:nth x 0)
                if (list? x) (&list:nth x 0)
                  .first x

        |rest $ quote
          defn rest (x)
            if (nil? x) nil
              if (list? x) (&list:rest x)
                .rest x

        |assoc $ quote
          defn assoc (x & args)
            if (nil? x) (raise "|assoc does not work on nil")
              if (tuple? x) (&tuple:assoc x & args)
                if (list? x) (&list:assoc x & args)
                  .assoc x & args

        |dissoc $ quote
          defn dissoc (x & args)
            if (nil? x) nil
              if (list? x) (&list:dissoc x & args)
                .dissoc x & args

        |concat $ quote
          defn concat (& args)
            if (&list:empty? args) ([])
              .concat (first args) & (rest args)

        |&list:map-pair $ quote
          defn &list:map-pair (xs f)
            if (list? xs)
              map xs $ defn %map-pair (pair)
                assert "|expected a pair" $ and (list? pair) $ = 2 $ count pair
                f (nth pair 0) (nth pair 1)
              raise "|expected list or map from `map-pair`"

        |&list:filter-pair $ quote
          defn &list:filter-pair (xs f)
            if (list? xs)
              &list:filter xs $ defn %filter-pair (pair)
                assert "|expected a pair" $ and (list? pair) $ = 2 $ count pair
                f (nth pair 0) (nth pair 1)
              raise "|expected list or map from `filter-pair`"

        |&list:flatten $ quote
          defn &list:flatten (xs)
            if (list? xs)
              &list:concat & $ map xs &list:flatten
              [] xs

        |keywordize-edn $ quote
          defn keywordize-edn (data)
            if (list? data)
              map data keywordize-edn
              if (map? data)
                map-kv data $ defn %keywordize (k v)
                  []
                    if (string? k) (turn-keyword k) k
                    keywordize-edn v
                , data

        |print-values $ quote
          defn print-values (& args)
            println & $ &list:map args pr-str

        |noted $ quote
          defmacro noted (x0 & xs)
            if (empty? xs) x0
              last xs
