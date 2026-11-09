{}
  :package |demo
  :entries $ {}
    :default $ {} (:init-fn 'demo.main/main!) (:reload-fn 'demo.main/main!) (:mode :js) (:target :browser)
      :modules $ []
  :files $ {}
    'demo.main $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
      :defs $ {}
        'Task $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Task (:title String) (:done Bool)
          :schema $ quote StructDef
        'c12 $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn c12 (t) $ update t :title $ fn (x) (+ 1 x)
          :schema $ quote $ Fn Task (-> Task)
        'c-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn c-ok (t) $ update t :title $ fn (x) (str x |!)
          :schema $ quote $ Fn Task (-> Task)
        'c-ret $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn c-ret (t) $ update t :title $ fn (x) (&str:count x)
          :schema $ quote $ Fn Task (-> Task)
        'Box $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Box ([] T) (:value T)
          :schema $ quote StructDef
        'box-inc $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn box-inc (b) $ update b :value $ fn (x) (+ x 1)
          :schema $ quote $ Fn (Box Number) (-> (Box Number))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () nil
          :schema $ quote $ Fn (-> Nil)
