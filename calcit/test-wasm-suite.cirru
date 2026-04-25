
{} (:about |Hand-authored WASM-target test suite — pulls in util plus pure-compute test modules and dispatches their main!. Edit via `cr edit`/`cr tree` once seeded.) (:package |test-wasm-suite)
  :configs $ {} (:init-fn |test-wasm-suite.main/main!) (:reload-fn |test-wasm-suite.main/reload!) (:version |0.0.0)
    :modules $ [] |./util.cirru |./test-cond.cirru |./test-math.cirru |./test-set.cirru |./test-tuple.cirru |./test-fn.cirru |./test-lens.cirru |./test-edn.cirru
  :entries $ {}
  :files $ {}
    |test-wasm-suite.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! ()
              test-cond/main!
              test-math/main!
              test-set/main!
              test-tuple/main!
              test-fn/main!
              test-lens/main!
              test-edn/main!
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () (main!)
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns test-wasm-suite.main $ :require
            [] test-cond.main :as test-cond
            [] test-math.main :as test-math
            [] test-set.main :as test-set
            [] test-tuple.main :as test-tuple
            [] test-fn.main :as test-fn
            [] test-lens.main :as test-lens
            [] test-edn.main :as test-edn
            [] test-algebra.main :as test-algebra
