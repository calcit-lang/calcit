
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-gynienic)
  :configs $ {} (:init-fn |test-gynienic.main/main!) (:reload-fn |test-gynienic.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-gynienic.lib $ %{} :FileEntry
      :defs $ {}
        |add-11 $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defmacro add-11 (a b)
              let
                  c 11
                println "|internal c:" a b c
                quasiquote $ do (println "|c is:" c)
                  [] (~ a) (~ b) c (~ c) (add-2 8)
          :examples $ []
        |add-2 $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn add-2 (x) (&+ x 2)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (:ns test-gynienic.lib)
    |test-gynienic.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () $ try-hygienic
          :examples $ []
        |try-hygienic $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn try-hygienic () (println "|Testing gynienic")
              let
                  c 4
                assert= (add-11 1 2) ([] 1 2 4 11 10)
                , true
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-gynienic.main $ :require
            [] test-gynienic.lib :refer $ [] add-11
