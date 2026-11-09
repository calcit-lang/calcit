{}
  :package |demo
  :entries $ {}
    :default $ {} (:init-fn 'demo.main/main!) (:reload-fn 'demo.main/main!) (:mode :js) (:target :browser)
      :type-slots $ {} (:dispatch-op 'demo.main/ViewOp)
      :modules $ []
  :files $ {}
    'demo.host $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.host
      :defs $ {}
        '*dispatch-op $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftype-slot :dispatch-op
          :schema $ quote Dynamic
        'accept! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn accept! (_on-op!) &unit
          :schema $ quote $ Fn (Fn *dispatch-op (-> Unit)) (-> Unit)
        'forward! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn forward! (op) (demo.main/on-op! op)
          :schema $ quote $ Fn *dispatch-op (-> Unit)
    'demo.main $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
      :defs $ {}
        'AppOp $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum AppOp (:reset)
          :schema $ quote EnumDef
        'NavOp $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum NavOp (:tab Tag)
          :schema $ quote EnumDef
        'ViewOp $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion ViewOp AppOp NavOp
          :schema $ quote UnionDef
        'on-op! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn on-op! (_op) &unit
          :schema $ quote $ Fn demo.main/ViewOp (-> Unit)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (demo.host/accept! on-op!)
          :schema $ quote $ Fn (-> Unit)
