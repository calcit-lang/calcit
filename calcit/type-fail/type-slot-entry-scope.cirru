{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |type-fail-type-slot-entry-scope)
  :configs $ {} (:init-fn |type-fail-type-slot-entry-scope.main/client-main!) (:reload-fn |type-fail-type-slot-entry-scope.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
    :server $ {} (:init-fn |type-fail-type-slot-entry-scope.main/server-main!) (:reload-fn |type-fail-type-slot-entry-scope.main/reload!) (:version |0.0.0)
  :files $ {}
    |type-fail-type-slot-entry-scope.main $ %{} :FileEntry
      :defs $ {}
        |ClientOp $ %{} :CodeEntry (:doc "|Client entry enum")
          :code $ quote
            defenum ClientOp
              :client/ping
          :examples $ []
          :schema :dynamic
        |server-main! $ %{} :CodeEntry (:doc "|Server entry binds the same slot name independently")
          :code $ quote
            defn server-main! () $ with-type-slot (:dispatch-op ServerOp)
              accept-op $ :: :server/ping
              , nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |ServerOp $ %{} :CodeEntry (:doc "|Server entry enum")
          :code $ quote
            defenum ServerOp
              :server/ping
          :examples $ []
          :schema :dynamic
        |accept-op $ %{} :CodeEntry (:doc "|Schema depends on the entry-bound type slot")
          :code $ quote
            defn accept-op (op) op
          :examples $ []
          :schema $ :: :fn
            {} (:return '*dispatch-op)
              :args $ [] '*dispatch-op
        |client-main! $ %{} :CodeEntry (:doc "|Client entry binds dispatch-op for client enums")
          :code $ quote
            defn client-main! () $ with-type-slot (:dispatch-op ClientOp)
              accept-op $ :: :client/ping
              , nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc "|Fixture for entry-scoped with-type-slot preprocessing")
        :code $ quote (ns type-fail-type-slot-entry-scope.main)