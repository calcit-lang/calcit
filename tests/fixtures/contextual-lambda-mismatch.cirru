{}
  :package |demo
  :entries $ {}
    :default $ {} (:init-fn 'demo.main/main!) (:reload-fn 'demo.main/main!) (:mode :native)
      :modules $ []
  :files $ {}
    'demo.main $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
      :defs $ {}
        'Task $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Task (:title String) (:done Bool)
          :schema $ quote StructDef
        'tail-lambda $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tail-lambda () $ fn (x) $ &+ x 1
          :schema $ quote $ Fn (-> (Fn Task (-> Number)))
        'let-tail $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn let-tail () $ let ((other (fn (y) y))) $ fn (x) $ &+ x 1
          :schema $ quote $ Fn (-> (Fn Task (-> Number)))
        'if-tail $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-tail (flag) $ if flag (fn (x) $ &+ x 1) (fn (x) 0)
          :schema $ quote $ Fn Bool (-> (Fn Task (-> Number)))
        'bound-once $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bound-once (ts) $ let ((step (fn (x) $ &+ x 1))) $ .map ts step
          :schema $ quote $ Fn (List Task) (-> (List Number))
        'bound-returned $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bound-returned () $ let ((step (fn (x) $ &+ x 1))) step
          :schema $ quote $ Fn (-> (Fn Task (-> Number)))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () $ let ((t (Task :title |abc :done true))) (println ((tail-lambda) t) ((let-tail) t) ((if-tail true) t) (bound-once ([] t)) ((bound-returned) t)) &unit
          :schema $ quote $ Fn (-> Unit)
