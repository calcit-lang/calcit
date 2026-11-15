
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Point $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Point (:x Number) (:impls PointOps)
          :examples $ []
          :schema $ quote StructDef
        'PointOps $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait PointOps
            .read-x $ Fn Self $ -> Number
          :examples $ []
          :schema $ quote Trait
        'PointOpsImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl Point PointOps
            .read-x $ fn (self) |not-a-number
          :examples $ []
          :schema $ quote Impl
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (identity PointOpsImpl) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
