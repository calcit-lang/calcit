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
          :code $ quote $ defn tail-lambda () $ fn (x) $ .len $ :title x
          :schema $ quote $ Fn (-> (Fn Task (-> Number)))
        'let-tail $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn let-tail (suffix) $ let ((other (fn (y) y))) $ fn (x) $ str (:title x) suffix
          :schema $ quote $ Fn String (-> (Fn Task (-> String)))
        'if-tail $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-tail (flag) $ if flag (fn (x) $ .len $ :title x) (fn (x) 0)
          :schema $ quote $ Fn Bool (-> (Fn Task (-> Number)))
        'bound-once $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bound-once (ts) $ let ((label (fn (x) $ str (:title x) |!))) $ .map ts label
          :schema $ quote $ Fn (List Task) (-> (List String))
        'bound-in-let $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bound-in-let (ts) $ let ((done? (fn (x) $ :done x)) (n (count ts))) $ .filter ts done?
          :schema $ quote $ Fn (List Task) (-> (List Task))
        'bound-returned $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn bound-returned () $ let ((size (fn (x) $ .len $ :title x))) size
          :schema $ quote $ Fn (-> (Fn Task (-> Number)))
        'both $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro both (flag xs f) $ quasiquote $ if ~flag (.map ~xs ~f) (.map (.reverse ~xs) ~f)
          :schema $ quote $ Macro (Expr Bool) (Expr Dynamic) (Expr Dynamic) (-> (Expr Dynamic))
        'dup-macro $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn dup-macro (ts flag) $ let ((label (fn (x) $ str (:title x) |!))) $ both flag ts label
          :schema $ quote $ Fn (List Task) Bool (-> (List String))
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () $ let ((t (Task :title |abc :done true))) (println ((tail-lambda) t) ((let-tail |?) t) ((if-tail false) t) (bound-once ([] t)) (count (bound-in-let ([] t))) ((bound-returned) t) (dup-macro ([] t) false)) &unit
          :schema $ quote $ Fn (-> Unit) (:features :io)
