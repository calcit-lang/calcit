
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-algebra) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-algebra.main/main!) (:mode :native) (:reload-fn 'test-algebra.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-algebra.main $ %{} :FileEntry
      :defs $ {}
        |AlgebraApply $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraApply $ .apply :fn
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraBind $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraBind $ .bind :fn
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraBox $ %{} :CodeEntry (:doc |)
          :code $ quote
            def AlgebraBox $ impl-traits AlgebraBox0 AlgebraBoxMapImpl AlgebraBoxBindImpl AlgebraBoxApplyImpl AlgebraBoxMappendImpl
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraBox0 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defstruct AlgebraBox0 $ :value 'Dynamic
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraBoxApplyImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxApplyImpl AlgebraApply $ .apply
              fn (box fs)
                let
                    f $ &struct:get fs :value
                  assoc box :value $ f (&struct:get box :value)
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraBoxBindImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxBindImpl AlgebraBind $ .bind
              fn (box f)
                f $ &struct:get box :value
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraBoxMapImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxMapImpl AlgebraMap $ .map
              fn (box f)
                assoc box :value $ f (&struct:get box :value)
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraBoxMappendImpl $ %{} :CodeEntry (:doc |)
          :code $ quote
            defimpl AlgebraBoxMappendImpl AlgebraMappend $ .mappend
              fn (a b)
                assoc a :value $ + (&struct:get a :value) (&struct:get b :value)
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraMap $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraMap $ .map :fn
          :examples $ []
          :schema $ :: 'Dynamic
        |AlgebraMappend $ %{} :CodeEntry (:doc |)
          :code $ quote
            deftrait AlgebraMappend $ .mappend :fn
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing algebra") (; "|Experimental code, to simulate usages like Monad") (test-map) (test-bind) (test-apply) (test-mappend)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
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
                assert= 12 $ get b2 :value
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-bind $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-bind () $ let
                b1 $ %{} AlgebraBox (:value 5)
              assert-traits b1 AlgebraBind
              let
                  b2 $ .bind b1
                    fn (x)
                      %{} AlgebraBox $ :value (+ x 20)
                assert= 25 $ get b2 :value
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-map $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map () $ let
                b1 $ %{} AlgebraBox (:value 2)
              assert-traits b1 AlgebraMap
              let
                  b2 $ .map b1
                    fn (x) (+ x 10)
                assert= 12 $ get b2 :value
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |test-mappend $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-mappend () $ let
                b1 $ %{} AlgebraBox (:value 3)
                b2 $ %{} AlgebraBox (:value 4)
              assert-traits b1 AlgebraMappend
              assert-traits b2 AlgebraMappend
              let
                  b3 $ .mappend b1 b2
                assert= 7 $ get b3 :value
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-algebra $ :require
            util.core :refer $ log-title inside-eval:
