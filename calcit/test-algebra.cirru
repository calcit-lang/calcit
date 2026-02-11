
{} (:package |test-algebra)
  :configs $ {} (:init-fn |test-algebra/main!) (:reload-fn |test-algebra/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-algebra.main $ %{} :FileEntry
      :defs $ {}
        |AlgebraApply $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraApply
              :apply :fn
        |AlgebraBind $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraBind
              :bind :fn
        |AlgebraMap $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraMap
              :map :fn
        |AlgebraMappend $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraMappend
              :mappend :fn
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing algebra") (; "\"Experimental code, to simulate usages like Monad") (test-map) (test-bind) (test-apply) (test-mappend)
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
        |test-apply $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-apply ()
              let
                  xs $ []
                assert-traits xs AlgebraApply
                assert= ([]) $ .apply xs ([] inc)
              let
                  ys $ [] 1 2 3
                assert-traits ys AlgebraApply
                assert= ([] 11 12 13 2 4 6)
                  .apply ys
                    []
                      fn (x) (+ x 10)
                      fn (x) (* x 2)
              let
                  f1 $ fn (x) (+ x 10)
                  f2 $ fn (y z) (* 2 y z)
                assert-traits f1 AlgebraApply
                assert-traits f2 AlgebraApply
                let
                    f3 $ .apply f1 f2
                  assert= 78 $ f3 3
        |test-bind $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-bind ()
              let
                  xs $ []
                assert-traits xs AlgebraBind
                assert= ([]) $ .bind xs inc
              let
                  ys $ [] 2 3
                assert-traits ys AlgebraBind
                assert= ([] 0 1 0 1 2)
                  .bind ys
                    fn (x) (range x)
              let
                  f1 $ fn (x) (+ x 10)
                  f2 $ fn (x y) (* 2 x y)
                assert-traits f1 AlgebraBind
                assert-traits f2 AlgebraBind
                let
                    f3 $ .bind f1 f2
                  assert= 78 $ f3 3
        |test-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map ()
              let
                  xs $ []
                assert-traits xs AlgebraMap
                assert= ([]) $ .map xs inc
              let
                  ys $ ' 1 2
                assert-traits ys AlgebraMap
                assert= ([] 11 12)
                  .map ys
                    fn (x) (+ x 10)
              let
                  m $ &{} :a 1 :b 2
                assert-traits m AlgebraMap
                assert= (&{} :a 2 :b 3)
                  .map m
                    fn (pair)
                      [] (first pair)
                        inc $ last pair
              let
                  f1 $ fn (x) (+ x 10)
                  f2 $ fn (x) (* x 2)
                assert-traits f1 AlgebraMap
                assert-traits f2 AlgebraMap
                let
                    f3 $ .map f1 f2
                  assert= 16 $ f3 3
        |test-mappend $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-mappend ()
              let
                  xs $ []
                assert-traits xs AlgebraMappend
                assert= ([]) $ .mappend xs ([])
              let
                  s1 |ab
                assert-traits s1 AlgebraMappend
                assert= |abcd $ .mappend s1 |cd
              let
                  xs $ [] 1 2
                assert-traits xs AlgebraMappend
                assert= ([] 1 2 3 4)
                  .mappend xs ([] 3 4)
              let
                  s1 $ #{} 1 2
                assert-traits s1 AlgebraMappend
                assert= (#{} 1 2 3 4)
                  .mappend s1 (#{} 3 4)
              let
                  m1 $ &{} :a 1
                assert-traits m1 AlgebraMappend
                assert= (&{} :a 1 :b 2)
                  .mappend m1 (&{} :b 2)
              let
                  f1 $ fn (x)
                    let
                        _ $ assert-type x :string
                      .slice x 1
                  f2 $ fn (x)
                    let
                        _ $ assert-type x :string
                      .slice x 0 $ dec (count x)
                assert-traits f1 AlgebraMappend
                assert-traits f2 AlgebraMappend
                let
                    f3 $ .mappend f1 f2
                  assert= |234123 $ f3 |1234
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-algebra $ :require
            util.core :refer $ log-title inside-eval:
