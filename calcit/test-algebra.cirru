
{} (:package |test-algebra)
  :configs $ {} (:init-fn |test-algebra.main/main!) (:reload-fn |test-algebra.main/reload!)
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
        |AlgebraBox0 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct AlgebraBox0
              :value :dynamic
        |AlgebraBoxMapImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxMapImpl AlgebraMap
              :map $ fn (box f)
                assoc box :value $ f (:value box)
        |AlgebraBoxBindImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxBindImpl AlgebraBind
              :bind $ fn (box f)
                f $ :value box
        |AlgebraBoxApplyImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxApplyImpl AlgebraApply
              :apply $ fn (box fs)
                let
                    f $ :value fs
                  assoc box :value $ f (:value box)
        |AlgebraBoxMappendImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxMappendImpl AlgebraMappend
              :mappend $ fn (a b)
                assoc a :value $ + (:value a) (:value b)
        |AlgebraBox $ %{} :CodeEntry (:doc |)
          :code $ quote
            def AlgebraBox $ impl-traits AlgebraBox0 AlgebraBoxMapImpl AlgebraBoxBindImpl AlgebraBoxApplyImpl AlgebraBoxMappendImpl
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
                  b1 $ %{} AlgebraBox (:value 3)
                  bf $ %{} AlgebraBox (:value $ fn (x) (* x 4))
                assert-traits b1 AlgebraApply
                assert-traits bf AlgebraApply
                let
                    b2 $ &trait-call AlgebraApply :apply b1 bf
                  assert= 12 $ :value b2
        |test-bind $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-bind ()
              let
                  b1 $ %{} AlgebraBox (:value 5)
                assert-traits b1 AlgebraBind
                let
                    b2 $ &trait-call AlgebraBind :bind b1 $ fn (x)
                      %{} AlgebraBox (:value $ + x 20)
                  assert= 25 $ :value b2
        |test-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map ()
              let
                  b1 $ %{} AlgebraBox (:value 2)
                assert-traits b1 AlgebraMap
                let
                    b2 $ &trait-call AlgebraMap :map b1 $ fn (x) (+ x 10)
                  assert= 12 $ :value b2
        |test-mappend $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-mappend ()
              let
                  b1 $ %{} AlgebraBox (:value 3)
                  b2 $ %{} AlgebraBox (:value 4)
                assert-traits b1 AlgebraMappend
                assert-traits b2 AlgebraMappend
                let
                    b3 $ &trait-call AlgebraMappend :mappend b1 b2
                  assert= 7 $ :value b3
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-algebra $ :require
            util.core :refer $ log-title inside-eval:
