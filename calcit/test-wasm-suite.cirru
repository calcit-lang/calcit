
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-wasm-suite) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-wasm-suite.main/main!) (:mode :native) (:reload-fn 'test-wasm-suite.main/reload!)
      :modules $ [] |./util.cirru |./test-cond.cirru |./test-math.cirru |./test-set.cirru |./test-tuple.cirru |./test-fn.cirru |./test-lens.cirru |./test-edn.cirru |./test-string.cirru |./test-nil.cirru
      :type-slots $ {}
  :files $ {}
    |test-wasm-suite.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (test-cond/main!) (test-math/main!) (test-set/main!) (test-tuple/main!) (test-fn/main!) (test-lens/main!) (test-edn/main!) (test-string/main!) (test-nil/main!)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () $ main!
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns test-wasm-suite.main $ :require ([] test-cond.main :as test-cond) ([] test-math.main :as test-math) ([] test-set.main :as test-set) ([] test-tuple.main :as test-tuple) ([] test-fn.main :as test-fn) ([] test-lens.main :as test-lens) ([] test-edn.main :as test-edn) ([] test-string.main :as test-string) (test-nil.main :as test-nil)
