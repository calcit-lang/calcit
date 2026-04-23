
{} (:about "|Machine-generated snapshot. AI AGENTS: never edit this file directly — changes will be overwritten on recompile. Inspect via `cr query`; modify via `cr edit` / `cr tree`. MANDATORY first step: run `cr docs agents --full`.") (:package |test-doc-smoke)
  :configs $ {} (:init-fn |test-doc-smoke.main/main!) (:reload-fn |test-doc-smoke.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-doc-smoke.main $ %{} :FileEntry
      :defs $ {}
        |DocEnum0 $ %{} :CodeEntry (:doc "|Doc smoke enum") (:schema nil)
          :code $ quote
            defenum DocEnum $ :ok :string
          :examples $ []
        |DocPerson0 $ %{} :CodeEntry (:doc "|Doc smoke struct") (:schema nil)
          :code $ quote
            defstruct DocPerson $ :name :string
          :examples $ []
        |DocTrait $ %{} :CodeEntry (:doc "|Doc smoke trait") (:schema nil)
          :code $ quote
            deftrait DocTrait $ .label :fn
          :examples $ []
        |DocTraitImpl $ %{} :CodeEntry (:doc "|Doc smoke impl") (:schema nil)
          :code $ quote
            defimpl DocTraitImpl DocTrait $ .label
              fn (x)
                str-spaced |doc $ :name x
          :examples $ []
        |main! $ %{} :CodeEntry (:doc "|Run docs smoke cases") (:schema nil)
          :code $ quote
            defn main! () (println "|Testing doc smoke cases...") (test-defimpl-order) (test-native-impl-new-dot-method) (test-assert-traits-local) (test-impl-traits-struct-enum-only) (println "|Doc smoke cases passed")
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler") (:schema nil)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |test-assert-traits-local $ %{} :CodeEntry (:doc "|assert-traits local first arg smoke") (:schema nil)
          :code $ quote
            defn test-assert-traits-local () $ let
                DocPerson $ impl-traits DocPerson0 DocTraitImpl
                p $ %{} DocPerson (:name |Alice)
              assert= p $ assert-traits p DocTrait
              assert= "|doc Alice" $ .label p
          :examples $ []
        |test-defimpl-order $ %{} :CodeEntry (:doc "|defimpl arg order smoke") (:schema nil)
          :code $ quote
            defn test-defimpl-order () $ assert= DocTrait (&impl:origin DocTraitImpl)
          :examples $ []
        |test-impl-traits-struct-enum-only $ %{} :CodeEntry (:doc "|impl-traits only accepts struct/enum definitions") (:schema nil)
          :code $ quote
            defn test-impl-traits-struct-enum-only ()
              let
                  DocPerson $ impl-traits DocPerson0 DocTraitImpl
                  DocEnum $ impl-traits DocEnum0 DocTraitImpl
                assert= true $ struct? DocPerson
                assert= true $ enum? DocEnum
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
        |test-native-impl-new-dot-method $ %{} :CodeEntry (:doc "|&impl::new accepts .method field keys") (:schema nil)
          :code $ quote
            defn test-native-impl-new-dot-method () $ let
                DotImpl $ &impl::new DocTrait
                  :: .label $ fn (x)
                    str-spaced |native-dot $ :name x
                DotPerson $ impl-traits DocPerson0 DotImpl
                p $ %{} DotPerson (:name |Bob)
              assert= DocTrait $ &impl:origin DotImpl
              assert= "|native-dot Bob" $ .label p
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-doc-smoke.main $ :require
            util.core :refer $ inside-eval:
