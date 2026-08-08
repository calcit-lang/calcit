
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-doc-smoke) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-doc-smoke.main/main!) (:mode :native) (:reload-fn 'test-doc-smoke.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-doc-smoke.main $ %{} 'FileEntry
      :defs $ {}
        |DocEnum0 $ %{} 'CodeEntry (:doc "|Doc smoke enum")
          :code $ quote
            defenum DocEnum $ :ok 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |DocPerson0 $ %{} 'CodeEntry (:doc "|Doc smoke struct")
          :code $ quote
            defstruct DocPerson $ :name 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |DocTrait $ %{} 'CodeEntry (:doc "|Doc smoke trait")
          :code $ quote
            deftrait DocTrait $ .label :fn
          :examples $ []
          :schema $ :: 'Dynamic
        |DocTraitImpl $ %{} 'CodeEntry (:doc "|Doc smoke impl")
          :code $ quote
            defimpl DocTraitImpl DocTrait $ .label
              fn (x)
                str-spaced |doc $ &struct:get x :name
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} 'CodeEntry (:doc "|Run docs smoke cases")
          :code $ quote
            defn main! () (println "|Testing doc smoke cases...") (test-defimpl-order) (test-native-impl-new-dot-method) (test-assert-traits-local) (test-impl-traits-struct-enum-only) (println "|Doc smoke cases passed")
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |test-assert-traits-local $ %{} 'CodeEntry (:doc "|assert-traits local first arg smoke")
          :code $ quote
            defn test-assert-traits-local () $ let
                DocPerson $ impl-traits DocPerson0 DocTraitImpl
                p $ %{} DocPerson (:name |Alice)
              assert= p $ assert-traits p DocTrait
              assert= "|doc Alice" $ p .label
          :examples $ []
          :schema $ :: 'Dynamic
        |test-defimpl-order $ %{} 'CodeEntry (:doc "|defimpl arg order smoke")
          :code $ quote
            defn test-defimpl-order () $ assert= (%some DocTrait) (impl-origin DocTraitImpl)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-impl-traits-struct-enum-only $ %{} 'CodeEntry (:doc "|impl-traits only accepts struct/enum definitions")
          :code $ quote
            defn test-impl-traits-struct-enum-only ()
              let
                  DocPerson $ impl-traits DocPerson0 DocTraitImpl
                  DocEnum $ impl-traits DocEnum0 DocTraitImpl
                assert= true $ struct-def? DocPerson
                assert= true $ enum-def? DocEnum
              let
                  msg $ try
                    do
                      impl-traits (%:: DocEnum0 :ok |done) DocTraitImpl
                      , |NO_ERROR
                    fn (e) (str e)
                assert= false $ &= msg |NO_ERROR
                inside-eval:
                  assert= true $ includes? msg |Expected:
                  assert= true $ includes? msg |Actual:
                  assert= true $ includes? msg |Fix:
          :examples $ []
          :schema $ :: 'Dynamic
        |test-native-impl-new-dot-method $ %{} 'CodeEntry (:doc "|&impl::new accepts .method field keys")
          :code $ quote
            defn test-native-impl-new-dot-method () $ let
                DotImpl $ &impl::new DocTrait
                  :: .label $ fn (x)
                    str-spaced |native-dot $ &struct:get x :name
                DotPerson $ impl-traits DocPerson0 DotImpl
                p $ %{} DotPerson (:name |Bob)
              assert= (%some DocTrait) (impl-origin DotImpl)
              assert= "|native-dot Bob" $ p .label
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-doc-smoke.main $ :require
            util.core :refer $ inside-eval:
