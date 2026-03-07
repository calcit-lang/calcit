
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |test-algebra)
  :configs $ {} (:init-fn |test-algebra.main/main!) (:reload-fn |test-algebra.main/reload!) (:version |0.0.0)
    :modules $ [] |./util.cirru
  :entries $ {}
  :files $ {}
    |test-algebra.main $ %{} :FileEntry
      :defs $ {}
        |AlgebraApply $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            deftrait AlgebraApply $ .apply :fn
          :examples $ []
        |AlgebraBind $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            deftrait AlgebraBind $ .bind :fn
          :examples $ []
        |AlgebraBox $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            def AlgebraBox $ impl-traits AlgebraBox0 AlgebraBoxMapImpl AlgebraBoxBindImpl AlgebraBoxApplyImpl AlgebraBoxMappendImpl
          :examples $ []
        |AlgebraBox0 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defstruct AlgebraBox0 $ :value :dynamic
          :examples $ []
        |AlgebraBoxApplyImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defimpl AlgebraBoxApplyImpl AlgebraApply $ .apply
              fn (box fs)
                let
                    f $ :value fs
                  assoc box :value $ f (:value box)
          :examples $ []
        |AlgebraBoxBindImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defimpl AlgebraBoxBindImpl AlgebraBind $ .bind
              fn (box f)
                f $ :value box
          :examples $ []
        |AlgebraBoxMapImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defimpl AlgebraBoxMapImpl AlgebraMap $ .map
              fn (box f)
                assoc box :value $ f (:value box)
          :examples $ []
        |AlgebraBoxMappendImpl $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defimpl AlgebraBoxMappendImpl AlgebraMappend $ .mappend
              fn (a b)
                assoc a :value $ + (:value a) (:value b)
          :examples $ []
        |AlgebraMap $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            deftrait AlgebraMap $ .map :fn
          :examples $ []
        |AlgebraMappend $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            deftrait AlgebraMappend $ .mappend :fn
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing algebra") (; "\"Experimental code, to simulate usages like Monad") (test-map) (test-bind) (test-apply) (test-mappend)
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
        |test-apply $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-apply () $ let
                b1 $ %{} AlgebraBox (:value 3)
                bf $ %{} AlgebraBox
                  :value $ fn (x) (* x 4)
              assert-traits b1 AlgebraApply
              assert-traits bf AlgebraApply
              let
                  b2 $ .apply b1 bf
                assert= 12 $ :value b2
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
        |test-bind $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-bind () $ let
                b1 $ %{} AlgebraBox (:value 5)
              assert-traits b1 AlgebraBind
              let
                  b2 $ .bind b1
                    fn (x)
                      %{} AlgebraBox $ :value (+ x 20)
                assert= 25 $ :value b2
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
        |test-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map () $ let
                b1 $ %{} AlgebraBox (:value 2)
              assert-traits b1 AlgebraMap
              let
                  b2 $ .map b1
                    fn (x) (+ x 10)
                assert= 12 $ :value b2
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
        |test-mappend $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-mappend () $ let
                b1 $ %{} AlgebraBox (:value 3)
                b2 $ %{} AlgebraBox (:value 4)
              assert-traits b1 AlgebraMappend
              assert-traits b2 AlgebraMappend
              let
                  b3 $ .mappend b1 b2
                assert= 7 $ :value b3
          :examples $ []
          :schema $ quote
            [] $ {} (:kind :fn)
              :args $ []
              :return :dynamic
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns test-algebra $ :require
            util.core :refer $ log-title inside-eval:
        :examples $ []
